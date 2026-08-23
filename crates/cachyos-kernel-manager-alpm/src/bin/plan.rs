//! Candidate transaction-planning tool — run inside the court VM.
//!
//! Reconstructs the oracle's ENTIRE execution chain for a user selection
//! (`km-window.cpp:48-71` + `Kernel::install`/`remove`/`commit_transaction`,
//! `kernel.cpp:89-163,288-304`) and renders the exact argv the oracle would
//! produce — the pacman commands, the `pacman -Qqs` probes, the
//! terminal-helper invocation, and the static-init probes (findmnt/chwd).
//!
//! The oracle side of the differential is the real GUI driven through
//! AT-SPI while traced with strace (`vm/in-vm/oracle-transact.sh`): the
//! execve chain witnessed there must equal the chain this tool models.
//!
//! Output schema `cachyos-km-candidate-plan-v1`:
//! ```json
//! {
//!   "schema": "cachyos-km-candidate-plan-v1",
//!   "probes":  [ {"argv": ["sh", "-c", "findmnt -ln -o FSTYPE /"]}, ... ],
//!   "probe_results": { "zfs_root": ..., "chwd_nvidia": ... },
//!   "selections": [ {"raw": "...", "install": [...], "remove": [...]} ],
//!   "commands": [ {"argv": ["pacman", "-S", "--needed", ...]}, ... ],
//!   "terminal": {"argv": ["...terminal-helper", "-s", "pkexec ...", "..."]}
//! }
//! ```
//!
//! Usage:
//!   cachyos-kernel-manager-plan --select <raw> [--select <raw> ...]

use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
use cachyos_kernel_manager_alpm::pacman_conf::{register_sections, MiniIni};
use cachyos_kernel_manager_core::companions_for;
use cachyos_kernel_manager_core::discovery::{CompanionNames, DiscoveredKernel};
use cachyos_kernel_manager_core::kernel::{
    kernel_headers_name, matches_headers_needle, DisplayVersion,
};
use cachyos_kernel_manager_core::selection::{KernelRow, SelectionState};
use cachyos_kernel_manager_exec::{exec_shell, terminal_helper_argv, Escalate};
use cachyos_kernel_manager_plan::{
    commit_commands, discover_aur, expand_aur_install, AurDiscoveryInput, HardwareProfile,
    PackageAction, Reason, TransactionPlan,
};
use serde_json::json;
use std::path::Path;

const PACMAN_CONF: &str = "/etc/pacman.conf";
const ALPM_ROOT: &str = "/";
const ALPM_DBPATH: &str = "/var/lib/pacman/";

/// The oracle's AUR discovery probe (`kernel.cpp:263`).
const PARU_PROBE: &str = "paru --aur -Sl | grep ' linux[^ ]*-headers' | awk '{print $2}'";

/// The oracle's static-init probe pipelines (byte-exact, `kernel.cpp:41-52`)
/// and the install-time module probes (`kernel.cpp:114-115`).
const FINDMNT_PROBE: &str = "findmnt -ln -o FSTYPE /";
const CHWD_PROBE: &str = "chwd --list-installed -d 2>/dev/null | grep Name | awk '{print $4}'";
const PKGQ_NVIDIA: &str = "pacman -Qqs '^linux-cachyos.*-nvidia$' 2>/dev/null";
const PKGQ_NVIDIA_OPEN: &str = "pacman -Qqs '^linux-cachyos.*-nvidia-open$' 2>/dev/null";

/// One witnessed exec event: the program basename + argv (path resolution is
/// fixture-controlled; the basename is the stable identity).
#[derive(Debug, Clone, serde::Serialize)]
struct ExecEvent {
    argv: Vec<String>,
}

impl ExecEvent {
    fn new(argv: &[&str]) -> Self {
        Self {
            argv: argv.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Run one of the oracle's `exec()` pipelines, record the exec chain the
/// oracle would produce, and return the captured stdout.
///
/// The oracle's `utils::exec` runs `popen(cmd, "r")` — `/bin/sh -c cmd` with
/// stdout captured. The recorded events model the sh plus the known inner
/// execs (the pipeline members), which is exactly what strace witnesses on
/// the oracle side.
fn probe(cmd: &str, events: &mut Vec<ExecEvent>) -> String {
    // glibc ≥ 2.44 popen argv: sh -c -- <cmd> (the `--` guards command
    // strings starting with `-`); the strace witness on the court VM shows
    // exactly this form.
    events.push(ExecEvent::new(&["sh", "-c", "--", cmd]));
    // inner execs of the known probe pipelines (mirrors the strace witness)
    if cmd.starts_with("findmnt") {
        events.push(ExecEvent::new(&["findmnt", "-ln", "-o", "FSTYPE", "/"]));
    } else if cmd.starts_with("chwd") {
        events.push(ExecEvent::new(&["chwd", "--list-installed", "-d"]));
        events.push(ExecEvent::new(&["grep", "Name"]));
        events.push(ExecEvent::new(&["awk", "{print $4}"]));
    } else if cmd.starts_with("pacman -Qqs") {
        // the needle is the 4th argv element (after pacman -Qqs)
        events.push(ExecEvent::new(&[
            "pacman",
            "-Qqs",
            if cmd.contains("nvidia-open") {
                "^linux-cachyos.*-nvidia-open$"
            } else {
                "^linux-cachyos.*-nvidia$"
            },
        ]));
    } else if cmd.starts_with("paru --aur") {
        events.push(ExecEvent::new(&["paru", "--aur", "-Sl"]));
        events.push(ExecEvent::new(&["grep", " linux[^ ]*-headers"]));
        events.push(ExecEvent::new(&["awk", "{print $2}"]));
    }
    exec_shell(cmd)
}

/// `utils::exec` result semantics: any non-empty stdout.
fn any_line(result: &str) -> bool {
    !result.is_empty()
}

/// The chwd result: any profile line starts with the DKMS family name.
fn chwd_matches(result: &str, family: &str) -> bool {
    result.lines().any(|l| l.starts_with(family))
}

fn main() {
    let owned: Vec<String> = std::env::args().skip(1).collect();
    let mut selects: Vec<String> = Vec::new();
    let mut aur_flag = false;
    let mut it = owned.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--select" => match it.next() {
                Some(v) => selects.push(v.clone()),
                None => {
                    eprintln!(
                        "usage: cachyos-kernel-manager-plan [--aur] --select <raw> [--select ...]"
                    );
                    std::process::exit(2);
                }
            },
            // `ENABLE_AUR_KERNELS` (meson `aur_kernels`): the shipped CMake
            // oracle has it OFF — `--aur` models the meson build.
            "--aur" => aur_flag = true,
            _ => {
                eprintln!(
                    "usage: cachyos-kernel-manager-plan [--aur] --select <raw> [--select ...]"
                );
                std::process::exit(2);
            }
        }
    }

    let handle =
        AlpmHandle::init(ALPM_ROOT, ALPM_DBPATH).unwrap_or_else(|e| panic!("alpm init: {e}"));
    let content = std::fs::read_to_string(PACMAN_CONF).unwrap_or_default();
    let ini = MiniIni::parse(&content);
    let registered = register_sections(&ini);
    for name in &registered {
        handle.register_syncdb(name);
    }

    // ------------------------------------------------------------------
    // 1. static-init probes (kernel.cpp:41-52) — findmnt, chwd, chwd
    // ------------------------------------------------------------------
    let mut probe_events: Vec<ExecEvent> = Vec::new();
    let findmnt_out = probe(FINDMNT_PROBE, &mut probe_events);
    let zfs_root = findmnt_out == "zfs";
    // the oracle evaluates TWO separate lambdas; chwd runs TWICE
    let chwd_1 = probe(CHWD_PROBE, &mut probe_events);
    let chwd_2 = probe(CHWD_PROBE, &mut probe_events);
    let chwd_nvidia = chwd_matches(&chwd_1, "nvidia-dkms") || chwd_matches(&chwd_2, "nvidia-dkms");
    let chwd_nvidia_open =
        chwd_matches(&chwd_1, "nvidia-open-dkms") || chwd_matches(&chwd_2, "nvidia-open-dkms");

    // ------------------------------------------------------------------
    // 2. discovery + rows (identical to the inspect tool)
    // ------------------------------------------------------------------
    let mut kernels = discover(&handle);

    // 2b. AUR discovery (`kernel.cpp:253-283`) — only when the feature is
    // enabled (`--aur`, the meson ENABLE_AUR_KERNELS; the shipped CMake
    // oracle has it OFF and never reaches this block).
    if aur_flag {
        let paru_available = Path::new("/sbin/paru").exists();
        let awk_available = Path::new("/sbin/awk").exists();
        // the probe runs only when BOTH tools exist AND repo kernels exist
        // (`kernel.cpp:262`); the model's gates are identical, so the
        // executed probe and the model must agree.
        let mut paru_output = String::new();
        let mut probe_ran = false;
        if paru_available && awk_available && !kernels.is_empty() {
            paru_output = probe(PARU_PROBE, &mut probe_events);
            probe_ran = true;
        }
        let discovery = discover_aur(&AurDiscoveryInput {
            enabled: true,
            paru_available,
            awk_available,
            repo_kernels: kernels.clone(),
            paru_output,
        });
        if let Some(msg) = &discovery.gate_message {
            eprintln!("{msg}");
        }
        debug_assert_eq!(discovery.probe_run, probe_ran, "AUR probe gate divergence");
        kernels.extend(discovery.aur_kernels);
    }

    let mut local_versions = std::collections::BTreeMap::new();
    // the FULL local db — the oracle's `alpm_db_get_pkg(localdb, name)`
    // lookups cover ANY installed package (nvidia-dkms, companions, ...),
    // not just the discovered kernels (kernel.cpp:102-109,143-161).
    let mut installed_set = std::collections::BTreeSet::new();
    for p in handle.local_packages() {
        installed_set.insert(p.name.clone());
    }
    for k in &kernels {
        if let Some(l) = handle.local_pkg(&k.name) {
            local_versions.insert(k.name.clone(), l.version.clone());
        }
    }
    let rows: Vec<KernelRow> = kernels
        .iter()
        .map(|k| {
            let local = handle.local_pkg(&k.name);
            // `Kernel::version` (`kernel.cpp:56-79`): AUR rows short-circuit
            // to `unknown-version` and NEVER flag an update (m_update stays
            // false — there is no sync-db version to compare).
            let update = if k.repo == "aur" {
                false
            } else {
                let display = match &local {
                    Some(l) => DisplayVersion::compute(Some(&l.version), &k.version, |a, b| {
                        handle.vercmp(a, b).cmp(&0)
                    }),
                    None => {
                        DisplayVersion::compute(None, &k.version, |_, _| std::cmp::Ordering::Equal)
                    }
                };
                display.update
            };
            let immutable = local
                .as_ref()
                .map(|l| match &l.installed_db {
                    None => true,
                    Some(db) => db == &k.repo,
                })
                .unwrap_or(false);
            KernelRow {
                raw: k.raw.clone(),
                name: k.name.clone(),
                installed: local.is_some(),
                immutable,
                update_available: update,
                // default checkbox state (init_kernels_tree_widget)
                checked: local.is_some() && immutable,
            }
        })
        .collect();
    let mut selection = SelectionState { rows };

    // ------------------------------------------------------------------
    // 3. apply the user's toggle (the AT-SPI driver flips the checkbox)
    // ------------------------------------------------------------------
    for raw in &selects {
        let idx = selection
            .rows
            .iter()
            .position(|r| &r.raw == raw)
            .unwrap_or_else(|| {
                eprintln!("plan: selection {raw:?} not found in discovery");
                std::process::exit(4);
            });
        selection.rows[idx].checked = !selection.rows[idx].checked;
    }
    let by_raw: std::collections::BTreeMap<String, DiscoveredKernel> =
        kernels.iter().map(|k| (k.raw.clone(), k.clone())).collect();

    // ------------------------------------------------------------------
    // 4. install/removal phases with the oracle's per-phase gates; the
    //    pacman -Qqs probes run INSIDE each install expansion
    //    (kernel.cpp:114-115), once per install-gated kernel. Per-kernel
    //    action lists are recorded for the selections report; the
    //    aggregate feeds commit_transaction.
    // ------------------------------------------------------------------
    let mut install_actions: Vec<PackageAction> = Vec::new();
    let mut remove_actions: Vec<PackageAction> = Vec::new();
    let mut module_results: Vec<(bool, bool)> = Vec::new();
    let mut per_selection: Vec<(String, Vec<PackageAction>, Vec<PackageAction>)> = Vec::new();
    // AUR installs accumulate in a SEPARATE list (the oracle's
    // g_aur_kernel_install_list) — they never enter the pacman lists.
    let mut aur_plan = TransactionPlan::default();

    for raw in selection.install_set() {
        let Some(kernel) = by_raw.get(&raw) else {
            continue;
        };
        if kernel.repo == "aur" {
            // `Kernel::install` (`kernel.cpp:90-95`): AUR kernels go to the
            // separate aur install list — no companions, no pacman.
            expand_aur_install(&mut aur_plan, &kernel.name);
            per_selection.push((
                raw.clone(),
                vec![PackageAction {
                    package: kernel.name.clone(),
                    reason: Reason::AurDependency,
                }],
                Vec::new(),
            ));
            continue;
        }
        let nvidia_modules = any_line(&probe(PKGQ_NVIDIA, &mut probe_events));
        let open_modules = any_line(&probe(PKGQ_NVIDIA_OPEN, &mut probe_events));
        module_results.push((nvidia_modules, open_modules));
        let hw = HardwareProfile {
            root_on_zfs: zfs_root,
            chwd_nvidia,
            chwd_nvidia_open,
            installed: installed_set.clone(),
            nvidia_modules_installed: nvidia_modules,
            nvidia_open_modules_installed: open_modules,
        };
        let mut one = TransactionPlan::default();
        one.expand_install(kernel, &hw);
        install_actions.extend(one.install.clone());
        per_selection.push((raw.clone(), one.install, Vec::new()));
    }
    for raw in selection.removal_set() {
        let Some(kernel) = by_raw.get(&raw) else {
            continue;
        };
        let hw = HardwareProfile {
            installed: installed_set.clone(),
            ..Default::default()
        };
        let mut one = TransactionPlan::default();
        one.expand_remove(kernel, &hw);
        remove_actions.extend(one.remove.clone());
        // merge into the matching per-selection record (a kernel can be in
        // BOTH phases: the update-available quirk)
        if let Some(entry) = per_selection.iter_mut().find(|(r, _, _)| r == &raw) {
            entry.2 = one.remove;
        } else {
            per_selection.push((raw.clone(), Vec::new(), one.remove));
        }
    }

    // ------------------------------------------------------------------
    // 5. commit_transaction: aggregate + render argv
    // ------------------------------------------------------------------
    let mut plan = TransactionPlan {
        install: install_actions,
        remove: remove_actions,
        aur_install: Vec::new(),
        aur_enabled: aur_flag,
        warnings: vec![],
    };
    plan.aur_install = std::mem::take(&mut aur_plan.aur_install);
    let commands = commit_commands(&plan);
    let command_argv: Vec<Vec<String>> = commands
        .iter()
        .map(|c| match c {
            cachyos_kernel_manager_exec::CommandPlan::InstallRepoPackages { packages, needed } => {
                cachyos_kernel_manager_exec::pacman_install_argv(packages, *needed)
            }
            cachyos_kernel_manager_exec::CommandPlan::RemovePackages { packages } => {
                cachyos_kernel_manager_exec::pacman_remove_argv(packages)
            }
            cachyos_kernel_manager_exec::CommandPlan::GitRefresh { .. } => {
                // the git refresh chain is filesystem-dependent and courted
                // separately (git-cache/lifecycle); the command-level model
                // renders the canonical first step the oracle runs:
                // `git clone <url> <name>` from the parent dir.
                vec!["git".into(), "clone".into()]
            }
            cachyos_kernel_manager_exec::CommandPlan::BuildAurPackage { .. } => {
                cachyos_kernel_manager_exec::makepkg_aur_argv()
            }
            _ => vec![],
        })
        .collect();

    // the terminal-helper invocations: EVERY command runs through
    // `runCmdTerminal` in the oracle — AUR builds NON-escalated
    // (`aur_kernel.cpp:53`), the repo install/remove ESCALATED
    // (`kernel.cpp:297,302`). `terminal` keeps the single-entry schema
    // (the FIRST command, matching the pre-AUR behavior); the per-command
    // escalation is recorded in `terminal_commands`.
    let terminal_argv: Option<Vec<String>> = command_argv.first().map(|argv| {
        let escalate = match commands.first() {
            Some(cachyos_kernel_manager_exec::CommandPlan::BuildAurPackage { .. }) => {
                Escalate::None
            }
            _ => Escalate::PkexecRootShell,
        };
        terminal_helper_argv(&argv.join(" "), escalate)
    });
    let terminal_commands: Vec<Option<Vec<String>>> = command_argv
        .iter()
        .zip(commands.iter())
        .map(|(argv, c)| {
            let escalate = match c {
                cachyos_kernel_manager_exec::CommandPlan::BuildAurPackage { .. } => Escalate::None,
                _ => Escalate::PkexecRootShell,
            };
            Some(terminal_helper_argv(&argv.join(" "), escalate))
        })
        .collect();

    let selections_json: Vec<serde_json::Value> = per_selection
        .iter()
        .map(|(raw, install, remove)| {
            json!({
                "raw": raw,
                "install": install.iter().map(action_json).collect::<Vec<_>>(),
                "remove": remove.iter().map(action_json).collect::<Vec<_>>(),
            })
        })
        .collect();

    let payload = json!({
        "schema": "cachyos-km-candidate-plan-v1",
        "probes": probe_events,
        "probe_results": {
            "zfs_root": zfs_root,
            "chwd_nvidia": chwd_nvidia,
            "chwd_nvidia_open": chwd_nvidia_open,
            "nvidia_modules_installed": module_results.last().map(|(a, _)| *a).unwrap_or(false),
            "nvidia_open_modules_installed": module_results.last().map(|(_, b)| *b).unwrap_or(false),
        },
        "aur": {
            "enabled": aur_flag,
            "aur_install": plan.aur_install,
        },
        "selections": selections_json,
        "commands": command_argv,
        "terminal": terminal_argv,
        "terminal_commands": terminal_commands,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
}

fn reason_str(reason: &Reason) -> &'static str {
    match reason {
        Reason::SelectedKernel => "SelectedKernel",
        Reason::RequiredHeaders => "RequiredHeaders",
        Reason::ZfsRootCompanion => "ZfsRootCompanion",
        Reason::NvidiaCompanion => "NvidiaCompanion",
        Reason::NvidiaOpenCompanion => "NvidiaOpenCompanion",
        Reason::ExistingModuleFamily => "ExistingModuleFamily",
        Reason::AurDependency => "AurDependency",
        Reason::RemovalCompanion => "RemovalCompanion",
    }
}

fn action_json(a: &PackageAction) -> serde_json::Value {
    json!({ "package": a.package, "reason": reason_str(&a.reason) })
}

/// Reconstruct `Kernel::get_kernels` (identical to the inspect tool).
fn discover(handle: &AlpmHandle) -> Vec<DiscoveredKernel> {
    let mut out = Vec::new();
    for db_name in handle.syncdb_names() {
        let packages = handle.db_packages(&db_name);
        let by_name = |name: &str| handle.db_get_pkg(&db_name, name);
        for pkg in &packages {
            if !matches_headers_needle(&pkg.name) || pkg.name.contains("linux-api-headers") {
                continue;
            }
            let headers = pkg.name.clone();
            let kernel_name = kernel_headers_name(&headers);
            let Some(kernel_pkg) = by_name(&kernel_name) else {
                continue;
            };
            let names = companions_for(&kernel_name);
            let companions = CompanionNames {
                zfs: names.zfs.filter(|n| by_name(n).is_some()),
                nvidia: names.nvidia.filter(|n| by_name(n).is_some()),
                nvidia_open: names.nvidia_open.filter(|n| by_name(n).is_some()),
            };
            out.push(DiscoveredKernel {
                repo: db_name.clone(),
                name: kernel_name.clone(),
                headers,
                version: kernel_pkg.version.clone(),
                companions,
                raw: format!("{db_name}/{kernel_name}"),
            });
        }
    }
    out
}
