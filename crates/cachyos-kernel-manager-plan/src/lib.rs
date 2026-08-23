//! Transaction planning.
//!
//! Reconstructs the oracle's package expansion (`Kernel::install`,
//! `Kernel::remove`, `commit_transaction`, `km-window.cpp:48-71`) as a pure,
//! inspectable object:
//!
//! ```text
//! USER INTENT → TRANSACTION PLAN → SYSTEM COMMAND → MACHINE SIDE EFFECT
//! ```
//!
//! Every implicit package carries a [`Reason`]. The plan is deterministic
//! for fixed input, which makes it courtable (`courts/transaction-plan/*`)
//! and property-testable.

#![forbid(unsafe_code)]

use cachyos_kernel_manager_core::selection::{KernelRow, SelectionState};
use cachyos_kernel_manager_core::DiscoveredKernel;
use cachyos_kernel_manager_exec::CommandPlan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Why a package is in the plan (directive §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    /// Directly selected by the user.
    SelectedKernel,
    /// The kernel's headers package.
    RequiredHeaders,
    /// Root filesystem is ZFS → the kernel's `-zfs` companion.
    ZfsRootCompanion,
    /// NVIDIA proprietary prebuilt companion.
    NvidiaCompanion,
    /// NVIDIA-open prebuilt companion.
    NvidiaOpenCompanion,
    /// The oracle's "modules already installed → reuse them" branch.
    ExistingModuleFamily,
    /// AUR kernel.
    AurDependency,
    /// Removal of an installed companion of a removed kernel.
    RemovalCompanion,
}

/// One planned package action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageAction {
    /// Package name as passed to pacman.
    pub package: String,
    /// Why it is planned.
    pub reason: Reason,
}

/// The inspectable plan (directive §13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionPlan {
    /// Install actions in the oracle's order (`[zfs?, nvidia?, kernel,
    /// headers]` per selected kernel).
    pub install: Vec<PackageAction>,
    /// Remove actions in the oracle's order (`kernel, headers, zfs, nvidia,
    /// nvidia-open`, installed-only companions).
    pub remove: Vec<PackageAction>,
    /// AUR kernel names in selection order — the oracle's
    /// `g_aur_kernel_install_list` (`kernel.cpp:36,91-94`). These are NOT
    /// pacman packages: `commit_transaction` builds each one FIRST via the
    /// git-refresh + `makepkg -sicf` path (`kernel.cpp:289-294`,
    /// `aur_kernel.cpp:42-55`), before the repo install/remove commands.
    #[serde(default)]
    pub aur_install: Vec<String>,
    /// `ENABLE_AUR_KERNELS` — the compile-time feature flag (meson
    /// `aur_kernels`, default off; the shipped CMake oracle has it OFF). The
    /// oracle's commit-time AUR block is `#ifdef`'d out when disabled
    /// (`kernel.cpp:289-294`), so a plan with `aur_enabled == false` NEVER
    /// emits AUR build commands — the list is inert, exactly like the
    /// non-existent `g_aur_kernel_install_list` in the shipped oracle.
    #[serde(default)]
    pub aur_enabled: bool,
    /// Human-readable explanations for non-obvious expansions.
    pub warnings: Vec<String>,
}

/// Hardware/environment facts the expansion depends on
/// (`kernel.cpp:41-52` — evaluated once per process by the oracle).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// `findmnt -ln -o FSTYPE /` == `zfs`.
    pub root_on_zfs: bool,
    /// Any chwd profile name starts with `nvidia-dkms`.
    pub chwd_nvidia: bool,
    /// Any chwd profile name starts with `nvidia-open-dkms`.
    pub chwd_nvidia_open: bool,
    /// Installed package names (the local database).
    pub installed: BTreeSet<String>,
    /// Non-empty result of `pacman -Qqs '^linux-cachyos.*-nvidia$'`.
    pub nvidia_modules_installed: bool,
    /// Non-empty result of `pacman -Qqs '^linux-cachyos.*-nvidia-open$'`.
    pub nvidia_open_modules_installed: bool,
}

impl TransactionPlan {
    /// Build the plan from the current selection and environment.
    ///
    /// Mirrors the oracle flow: compute the shared change list, then the
    /// install phase (`install_packages` → `Kernel::install`) and removal
    /// phase (`remove_packages` → `Kernel::remove`).
    pub fn from_selection(
        selection: &SelectionState,
        hardware: &HardwareProfile,
        kernels_by_raw: &std::collections::BTreeMap<String, DiscoveredKernel>,
    ) -> TransactionPlan {
        let mut plan = TransactionPlan::default();

        for raw in selection.install_set() {
            let Some(kernel) = kernels_by_raw.get(&raw) else {
                continue;
            };
            if kernel.repo == "aur" {
                // `Kernel::install` (`kernel.cpp:89-95`): AUR kernels bypass
                // the pacman expansion entirely — the name goes to the AUR
                // build list, no companions.
                expand_aur_install(&mut plan, &kernel.name);
                continue;
            }
            plan.expand_install(kernel, hardware);
        }
        for raw in selection.removal_set() {
            let Some(kernel) = kernels_by_raw.get(&raw) else {
                continue;
            };
            plan.expand_remove(kernel, hardware);
        }
        plan
    }

    /// `Kernel::install` (`kernel.cpp:89-135`).
    pub fn expand_install(&mut self, kernel: &DiscoveredKernel, hardware: &HardwareProfile) {
        // ZFS companion first.
        if hardware.root_on_zfs {
            if let Some(zfs) = &kernel.companions.zfs {
                self.install.push(PackageAction {
                    package: zfs.clone(),
                    reason: Reason::ZfsRootCompanion,
                });
            }
        }

        let nvidia_dkms_installed = hardware.installed.contains("nvidia-dkms");
        let nvidia_open_dkms_installed = hardware.installed.contains("nvidia-open-dkms");
        let dkms_modules_not_installed = !nvidia_dkms_installed && !nvidia_open_dkms_installed;

        // "if we have any of the modules already installed, then just use
        // whatever is installed. skipping chwd detection" — the reason
        // record distinguishes the branch that produced the decision
        // (chwd profile vs already-installed module family).
        let mut should_install_nvidia = hardware.chwd_nvidia && kernel.companions.nvidia.is_some();
        let mut should_install_nvidia_open =
            hardware.chwd_nvidia_open && kernel.companions.nvidia_open.is_some();
        let mut from_module_family = false;

        if hardware.nvidia_open_modules_installed && kernel.companions.nvidia_open.is_some() {
            should_install_nvidia_open = true;
            should_install_nvidia = false;
            from_module_family = true;
        } else if hardware.nvidia_modules_installed && kernel.companions.nvidia.is_some() {
            should_install_nvidia_open = false;
            should_install_nvidia = true;
            from_module_family = true;
        }

        let reason = if from_module_family {
            Reason::ExistingModuleFamily
        } else if should_install_nvidia_open {
            Reason::NvidiaOpenCompanion
        } else {
            Reason::NvidiaCompanion
        };
        if dkms_modules_not_installed && should_install_nvidia_open {
            self.install.push(PackageAction {
                package: kernel
                    .companions
                    .nvidia_open
                    .clone()
                    .expect("checked above"),
                reason,
            });
        } else if dkms_modules_not_installed && should_install_nvidia {
            self.install.push(PackageAction {
                package: kernel.companions.nvidia.clone().expect("checked above"),
                reason,
            });
        }

        self.install.push(PackageAction {
            package: kernel.name.clone(),
            reason: Reason::SelectedKernel,
        });
        self.install.push(PackageAction {
            package: kernel.headers.clone(),
            reason: Reason::RequiredHeaders,
        });
    }

    /// `Kernel::remove` (`kernel.cpp:137-163`) + the commit ordering
    /// (kernel first, then installed companions).
    pub fn expand_remove(&mut self, kernel: &DiscoveredKernel, hardware: &HardwareProfile) {
        self.remove.push(PackageAction {
            package: kernel.name.clone(),
            reason: Reason::SelectedKernel,
        });
        // append_to_removal_list(headers) then the companion modules, each
        // only if installed locally (kernel.cpp:143-161).
        if hardware.installed.contains(&kernel.headers) {
            self.remove.push(PackageAction {
                package: kernel.headers.clone(),
                reason: Reason::RemovalCompanion,
            });
        }
        for companion in [
            &kernel.companions.zfs,
            &kernel.companions.nvidia,
            &kernel.companions.nvidia_open,
        ]
        .into_iter()
        .flatten()
        {
            if hardware.installed.contains(companion) {
                self.remove.push(PackageAction {
                    package: companion.clone(),
                    reason: Reason::RemovalCompanion,
                });
            }
        }
    }
}

/// Build the `raw → DiscoveredKernel` map the planner consumes.
pub fn kernels_by_raw(
    kernels: &[DiscoveredKernel],
) -> std::collections::BTreeMap<String, DiscoveredKernel> {
    kernels.iter().map(|k| (k.raw.clone(), k.clone())).collect()
}

/// Derive the oracle's tree-row flags from discovery + local db + installed
/// provenance. `installed_db` is the `HAVE_ALPM_INSTALLED_DB` provenance
/// (empty string when unknown).
/// Build discovery-order selection rows for the plan tests.
///
/// FIXTURE HELPER ONLY (review seam #4): the `update_available` flag needs
/// the local vs sync version comparison (vercmp), which lives in the alpm
/// layer — the plan crate cannot compute it (layering: core/plan never
/// depend on alpm). The REAL plan input rows are built by the alpm layer's
/// plan CLI (alpm/src/bin/plan.rs) with the real vercmp, so this helper
/// hardcodes `update_available: false` and can never cover the update quirk
/// (the BOTH `-S --needed` and `-Rsn` path courted by
/// kernel-removal/update-available-execute). Do NOT build production
/// selection state from this function.
pub fn to_rows(
    kernels: &[DiscoveredKernel],
    local_versions: &std::collections::BTreeMap<String, (String, String)>,
) -> Vec<KernelRow> {
    kernels
        .iter()
        .map(|k| {
            let installed = local_versions.contains_key(&k.name);
            let installed_db = local_versions
                .get(&k.name)
                .map(|(db, _)| db.as_str())
                .unwrap_or("");
            let immutable = installed && (installed_db.is_empty() || installed_db == k.repo);
            // the update flag is the alpm layer's vercmp job; this fixture
            // helper cannot compute it (see the fn docs)
            KernelRow {
                raw: k.raw.clone(),
                name: k.name.clone(),
                installed,
                immutable,
                update_available: false,
                checked: installed && immutable,
            }
        })
        .collect()
}

/// AUR selection support (`kernel.cpp:89-95`): selecting an AUR kernel adds
/// it to the aur install list without companions. The name is recorded in
/// [`TransactionPlan::aur_install`] — the oracle's separate
/// `g_aur_kernel_install_list` — NOT in `install` (AUR kernels are never
/// passed to `pacman -S`).
pub fn expand_aur_install(plan: &mut TransactionPlan, aur_kernel_name: &str) {
    plan.aur_install.push(aur_kernel_name.to_string());
}

/// The environment facts the AUR discovery block branches on
/// (`kernel.cpp:253-283`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AurDiscoveryInput {
    /// `ENABLE_AUR_KERNELS` — the compile-time feature flag (meson
    /// `aur_kernels`, default off; the shipped CMake oracle has it OFF).
    pub enabled: bool,
    /// `fs::exists("/sbin/paru")`.
    pub paru_available: bool,
    /// `fs::exists("/sbin/awk")`.
    pub awk_available: bool,
    /// The repo kernels discovered so far (`kernels`, in order) — AUR rows
    /// dedup against ALL of them by name (including earlier AUR rows).
    pub repo_kernels: Vec<DiscoveredKernel>,
    /// The raw stdout of `paru --aur -Sl | grep ' linux[^ ]*-headers' |
    /// awk '{print $2}'` (the oracle's `utils::exec` result, one trailing
    /// newline already stripped).
    pub paru_output: String,
}

/// The result of the oracle's AUR discovery block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AurDiscoveryResult {
    /// Whether the oracle executed the paru pipeline (observable in the
    /// strace witness).
    pub probe_run: bool,
    /// The AUR rows appended to discovery, in order
    /// (`repo="aur"`, `version="unknown-version"`, `raw="aur/<name>"`).
    pub aur_kernels: Vec<DiscoveredKernel>,
    /// The stderr gate message when paru/awk is missing (`kernel.cpp:258`).
    pub gate_message: Option<String>,
}

/// `Kernel::get_kernels` AUR block (`kernel.cpp:253-283`) as a pure
/// function. Byte-exact reconstruction:
///
/// - when the feature is OFF the whole block is compiled out — no probe, no
///   rows, no message (the shipped oracle);
/// - when ON: `is_paru_installed` starts `true`; if EITHER `/sbin/paru` or
///   `/sbin/awk` is missing, the stderr message is printed and
///   `is_paru_installed = false`;
/// - the probe runs only when `!kernels.empty() && is_paru_installed`;
/// - each probe line (non-empty, split on `\n` — `make_multiline`) has ALL
///   `-headers` occurrences removed (`replace_all`), names already present
///   (by name, repo-less) are skipped, and the row is appended with
///   `repo="aur"`, `version="unknown-version"`, `raw="aur/<name>"`.
pub fn discover_aur(input: &AurDiscoveryInput) -> AurDiscoveryResult {
    if !input.enabled {
        return AurDiscoveryResult {
            probe_run: false,
            aur_kernels: Vec::new(),
            gate_message: None,
        };
    }
    let paru_installed = input.paru_available && input.awk_available;
    if !paru_installed {
        return AurDiscoveryResult {
            probe_run: false,
            aur_kernels: Vec::new(),
            gate_message: Some(
                "Paru and/or AWK are not installed! Disabling AUR kernels support".to_string(),
            ),
        };
    }
    let mut aur_kernels = Vec::new();
    if input.repo_kernels.is_empty() {
        return AurDiscoveryResult {
            probe_run: false,
            aur_kernels,
            gate_message: None,
        };
    }
    // make_multiline: split on '\n', empty segments dropped.
    let names: Vec<DiscoveredKernel> = input
        .paru_output
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(|header| {
            let mut name = header.to_string();
            // replace_all(inout, "-headers", "") — every occurrence.
            while let Some(pos) = name.find("-headers") {
                name.replace_range(pos..pos + "-headers".len(), "");
            }
            DiscoveredKernel {
                repo: "aur".to_string(),
                name: name.clone(),
                headers: header.to_string(),
                version: "unknown-version".to_string(),
                companions: Default::default(),
                raw: format!("aur/{name}"),
            }
        })
        .collect();
    for k in names {
        let already = input
            .repo_kernels
            .iter()
            .chain(aur_kernels.iter())
            .any(|existing| existing.name == k.name);
        if !already {
            aur_kernels.push(k);
        }
    }
    AurDiscoveryResult {
        probe_run: true,
        aur_kernels,
        gate_message: None,
    }
}

/// `commit_transaction` (`kernel.cpp:288-304`): turn a set of plans into
/// the exact command sequence.
///
/// Oracle order:
/// 1. AUR installs FIRST (`kernel.cpp:289-294`): per name in selection
///    order — skip any name containing `headers` (`aur_kernel.cpp:46-48`,
///    `std::ranges::search` substring match), git-refresh
///    `~/.cache/cachyos-km/aur_pkgbuilds/<name>` from
///    `https://aur.archlinux.org/<name>.git` (`aur_kernel.cpp:32-36`), then
///    `makepkg -sicf --cleanbuild --skipchecksums` (`aur_kernel.cpp:53`),
/// 2. `pacman -S --needed <install list>` if any,
/// 3. `pacman -Rsn <remove list>` if any.
///
/// The install and remove lists are aggregated across ALL selected kernels
/// (`join_vec(list, " ")`), so multiple selections produce ONE install
/// command and ONE removal command.
pub fn commit_commands(plan: &TransactionPlan) -> Vec<CommandPlan> {
    let mut commands = Vec::new();
    if plan.aur_enabled {
        for name in &plan.aur_install {
            if name.contains("headers") {
                continue;
            }
            let dir = format!("~/.cache/cachyos-km/aur_pkgbuilds/{name}");
            commands.push(CommandPlan::GitRefresh {
                url: format!("https://aur.archlinux.org/{name}.git"),
                dir: dir.clone(),
            });
            // the build CARRIES its own cwd (audit P1: the runtime must
            // never infer it from neighboring commands — two AUR selections
            // used to make every build run in the LAST refresh's dir).
            commands.push(CommandPlan::BuildAurPackage { dir });
        }
    }
    if !plan.install.is_empty() {
        let packages: Vec<String> = plan.install.iter().map(|a| a.package.clone()).collect();
        commands.push(CommandPlan::InstallRepoPackages {
            packages,
            needed: true,
        });
    }
    if !plan.remove.is_empty() {
        let packages: Vec<String> = plan.remove.iter().map(|a| a.package.clone()).collect();
        commands.push(CommandPlan::RemovePackages { packages });
    }
    commands
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // test fixtures mutate defaults deliberately
mod tests {
    use super::*;
    use cachyos_kernel_manager_core::{discover_kernels, DbPackage, SyncDb};

    fn db(name: &str, pkgs: &[&str]) -> SyncDb {
        SyncDb {
            name: name.to_string(),
            packages: pkgs
                .iter()
                .map(|p| DbPackage {
                    name: p.to_string(),
                    version: "1.0-1".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn plain_install_is_kernel_then_headers() {
        let kernels =
            discover_kernels(&[db("cachyos", &["linux-cachyos", "linux-cachyos-headers"])]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile::default();
        let by_raw = kernels_by_raw(&kernels);
        let plan = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        assert_eq!(plan.install.len(), 2);
        assert_eq!(plan.install[0].package, "linux-cachyos");
        assert_eq!(plan.install[0].reason, Reason::SelectedKernel);
        assert_eq!(plan.install[1].package, "linux-cachyos-headers");
        assert_eq!(plan.install[1].reason, Reason::RequiredHeaders);
    }

    #[test]
    fn zfs_root_adds_zfs_companion_first() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile {
            root_on_zfs: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert_eq!(plan.install[0].package, "linux-cachyos-zfs");
        assert_eq!(plan.install[0].reason, Reason::ZfsRootCompanion);
        assert_eq!(plan.install[1].package, "linux-cachyos");
    }

    #[test]
    fn zfs_only_when_root_on_zfs() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;
        let hw = HardwareProfile {
            root_on_zfs: false,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-zfs"));
    }

    #[test]
    fn nvidia_matrix_chwd_and_dkms() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        // chwd says nvidia-dkms, no dkms installed -> prebuilt nvidia added
        let hw = HardwareProfile {
            chwd_nvidia: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia" && a.reason == Reason::NvidiaCompanion));

        // dkms installed -> no prebuilt
        let hw = HardwareProfile {
            chwd_nvidia: true,
            installed: ["nvidia-dkms".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));

        // no chwd -> nothing
        let plan = TransactionPlan::from_selection(
            &sel,
            &HardwareProfile::default(),
            &kernels_by_raw(&kernels),
        );
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));
    }

    #[test]
    fn nvidia_module_family_precedence() {
        // prebuilt nvidia installed -> prefer it even without chwd
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
                "linux-cachyos-nvidia-open",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile {
            nvidia_modules_installed: true,
            chwd_nvidia_open: true, // would suggest open, but closed wins
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia"));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia-open"));

        let hw = HardwareProfile {
            nvidia_open_modules_installed: true,
            chwd_nvidia: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia-open"));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));
    }

    #[test]
    fn removal_includes_only_installed_companions() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        // kernel + headers + zfs installed; installed-db == repo -> the row
        // is immutable and checked by default
        let mut local = std::collections::BTreeMap::new();
        local.insert(
            "linux-cachyos".to_string(),
            ("cachyos".to_string(), "6.14.1-1".to_string()),
        );
        sel.rows = to_rows(&kernels, &local);
        assert!(sel.rows[0].installed);
        assert!(sel.rows[0].immutable);
        assert!(sel.rows[0].checked);
        // user unchecks -> removal
        sel.rows[0].checked = false;

        // headers and zfs installed -> both removed
        let hw = HardwareProfile {
            installed: ["linux-cachyos-headers".into(), "linux-cachyos-zfs".into()]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        let pkgs: Vec<&str> = plan.remove.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(
            pkgs,
            vec![
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs"
            ]
        );

        // zfs not installed -> not removed
        let hw = HardwareProfile {
            installed: ["linux-cachyos-headers".into()].into_iter().collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        let pkgs: Vec<&str> = plan.remove.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(pkgs, vec!["linux-cachyos", "linux-cachyos-headers"]);
    }

    #[test]
    fn plan_is_deterministic() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;
        let hw = HardwareProfile {
            root_on_zfs: true,
            chwd_nvidia: true,
            ..Default::default()
        };
        let by_raw = kernels_by_raw(&kernels);
        let p1 = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        let p2 = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        assert_eq!(p1, p2);
        // install order: zfs, nvidia, kernel, headers
        let order: Vec<&str> = p1.install.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "linux-cachyos-zfs",
                "linux-cachyos-nvidia",
                "linux-cachyos",
                "linux-cachyos-headers"
            ]
        );
    }

    #[test]
    fn module_family_reason_is_recorded() {
        // the decision came from the pacman -Qqs branch (modules already
        // installed), NOT from a chwd profile: reason must say so
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;
        let hw = HardwareProfile {
            nvidia_modules_installed: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        let nvidia = plan
            .install
            .iter()
            .find(|a| a.package == "linux-cachyos-nvidia")
            .expect("nvidia companion planned");
        assert_eq!(nvidia.reason, Reason::ExistingModuleFamily);
    }

    #[test]
    fn commit_commands_aggregate_install_then_remove() {
        use cachyos_kernel_manager_exec::{pacman_install_argv, pacman_remove_argv, CommandPlan};
        // install kernel + headers, remove kernel (upgrade quirk)
        let plan = TransactionPlan {
            install: vec![
                PackageAction {
                    package: "linux-cachyos".into(),
                    reason: Reason::SelectedKernel,
                },
                PackageAction {
                    package: "linux-cachyos-headers".into(),
                    reason: Reason::RequiredHeaders,
                },
            ],
            remove: vec![PackageAction {
                package: "linux-cachyos".into(),
                reason: Reason::SelectedKernel,
            }],
            aur_install: vec![],
            aur_enabled: false,
            warnings: vec![],
        };
        let commands = commit_commands(&plan);
        assert_eq!(commands.len(), 2);
        match &commands[0] {
            CommandPlan::InstallRepoPackages { packages, needed } => {
                assert!(needed);
                assert_eq!(
                    packages,
                    &vec![
                        "linux-cachyos".to_string(),
                        "linux-cachyos-headers".to_string()
                    ]
                );
                assert_eq!(
                    pacman_install_argv(packages, *needed),
                    vec![
                        "pacman",
                        "-S",
                        "--needed",
                        "linux-cachyos",
                        "linux-cachyos-headers"
                    ]
                );
            }
            other => panic!("expected install command, got {other:?}"),
        }
        match &commands[1] {
            CommandPlan::RemovePackages { packages } => {
                assert_eq!(
                    pacman_remove_argv(packages),
                    vec!["pacman", "-Rsn", "linux-cachyos"]
                );
            }
            other => panic!("expected remove command, got {other:?}"),
        }
        // empty plan -> no commands
        assert!(commit_commands(&TransactionPlan::default()).is_empty());
    }

    #[test]
    fn aur_commit_builds_each_kernel_first() {
        use cachyos_kernel_manager_exec::{makepkg_aur_argv, CommandPlan};
        let plan = TransactionPlan {
            aur_install: vec!["linux-cachyos-zen".into(), "linux-cachyos-rc".into()],
            aur_enabled: true,
            install: vec![PackageAction {
                package: "linux-cachyos".into(),
                reason: Reason::SelectedKernel,
            }],
            remove: vec![],
            warnings: vec![],
        };
        let commands = commit_commands(&plan);
        assert_eq!(commands.len(), 5); // 2 AUR kernels x (refresh+build) + pacman
        match &commands[0] {
            CommandPlan::GitRefresh { url, dir } => {
                assert_eq!(url, "https://aur.archlinux.org/linux-cachyos-zen.git");
                assert_eq!(dir, "~/.cache/cachyos-km/aur_pkgbuilds/linux-cachyos-zen");
            }
            other => panic!("expected git refresh first, got {other:?}"),
        }
        assert_eq!(
            commands[1],
            CommandPlan::BuildAurPackage {
                dir: "~/.cache/cachyos-km/aur_pkgbuilds/linux-cachyos-zen".into()
            }
        );
        match &commands[2] {
            CommandPlan::GitRefresh { url, .. } => {
                assert_eq!(url, "https://aur.archlinux.org/linux-cachyos-rc.git");
            }
            other => panic!("expected second git refresh, got {other:?}"),
        }
        assert_eq!(
            commands[3],
            CommandPlan::BuildAurPackage {
                dir: "~/.cache/cachyos-km/aur_pkgbuilds/linux-cachyos-rc".into()
            }
        );
        match &commands[4] {
            CommandPlan::InstallRepoPackages { packages, .. } => {
                // the AUR name must NOT leak into pacman -S
                assert_eq!(packages, &vec!["linux-cachyos".to_string()]);
            }
            other => panic!("expected install last, got {other:?}"),
        }
        // argv rendering: makepkg -sicf --cleanbuild --skipchecksums
        assert_eq!(
            makepkg_aur_argv(),
            vec!["makepkg", "-sicf", "--cleanbuild", "--skipchecksums"]
        );
    }

    #[test]
    fn aur_commit_skips_names_containing_headers() {
        // aur_kernel.cpp:46-48 — `std::ranges::search(name, "headers")`
        // substring match: such names are skipped at build time.
        let plan = TransactionPlan {
            aur_install: vec!["linux-cachyos".into(), "linux-cachyos-lts-headers".into()],
            aur_enabled: true,
            install: vec![],
            remove: vec![],
            warnings: vec![],
        };
        let commands = commit_commands(&plan);
        assert_eq!(commands.len(), 2); // refresh+build for the non-headers name only
        assert!(matches!(commands[0], CommandPlan::GitRefresh { .. }));
        assert_eq!(
            commands[1],
            CommandPlan::BuildAurPackage {
                dir: "~/.cache/cachyos-km/aur_pkgbuilds/linux-cachyos".into()
            }
        );
    }

    #[test]
    fn aur_commit_is_inert_when_feature_disabled() {
        // the oracle's commit-time AUR block is `#ifdef ENABLE_AUR_KERNELS`
        // (kernel.cpp:289-294) — the shipped oracle (flag OFF) NEVER emits
        // AUR build commands, even for a non-empty list.
        let plan = TransactionPlan {
            aur_install: vec!["linux-cachyos-zen".into()],
            aur_enabled: false,
            install: vec![PackageAction {
                package: "linux-cachyos".into(),
                reason: Reason::SelectedKernel,
            }],
            remove: vec![],
            warnings: vec![],
        };
        let commands = commit_commands(&plan);
        assert_eq!(commands.len(), 1); // pacman only
        assert!(matches!(
            commands[0],
            CommandPlan::InstallRepoPackages { ref packages, .. } if packages == &vec!["linux-cachyos".to_string()]
        ));
        // serde: the flag defaults to false (old plans / shipped behavior)
        let back: TransactionPlan =
            serde_json::from_str("{\"install\": [], \"remove\": [], \"warnings\": []}").unwrap();
        assert!(!back.aur_enabled);
        assert!(back.aur_install.is_empty());
    }

    #[test]
    fn aur_selection_routes_to_aur_install_not_pacman_install() {
        let repo_kernels =
            discover_kernels(&[db("cachyos", &["linux-cachyos", "linux-cachyos-headers"])]);
        let mut all = repo_kernels.clone();
        all.push(DiscoveredKernel {
            repo: "aur".into(),
            name: "linux-cachyos-zen".into(),
            headers: "linux-cachyos-zen-headers".into(),
            version: "unknown-version".into(),
            companions: Default::default(),
            raw: "aur/linux-cachyos-zen".into(),
        });
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&all, &Default::default());
        // AUR rows are mutable (installed-db provenance "local" != "aur") and
        // unchecked by default; checking one makes it an install candidate.
        let aur_idx = sel
            .rows
            .iter()
            .position(|r| r.raw == "aur/linux-cachyos-zen")
            .unwrap();
        sel.rows[aur_idx].checked = true;
        sel.rows[0].checked = true; // repo kernel too

        let plan = TransactionPlan::from_selection(
            &sel,
            &HardwareProfile::default(),
            &kernels_by_raw(&all),
        );
        assert_eq!(plan.aur_install, vec!["linux-cachyos-zen".to_string()]);
        let install_pkgs: Vec<&str> = plan.install.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(install_pkgs, vec!["linux-cachyos", "linux-cachyos-headers"]);
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-zen"));
    }

    #[test]
    fn discover_aur_matrix() {
        use cachyos_kernel_manager_core::discovery::CompanionNames;
        let repo = |name: &str| DiscoveredKernel {
            repo: "cachyos".into(),
            name: name.into(),
            headers: format!("{name}-headers"),
            version: "1.0-1".into(),
            companions: CompanionNames::default(),
            raw: format!("cachyos/{name}"),
        };
        let repo_kernels = vec![repo("linux-cachyos")];

        // feature OFF -> nothing, even with paru present (shipped oracle)
        let off = discover_aur(&AurDiscoveryInput {
            enabled: false,
            paru_available: true,
            awk_available: true,
            repo_kernels: repo_kernels.clone(),
            paru_output: "linux-cachyos-zen-headers\n".into(),
        });
        assert!(!off.probe_run);
        assert!(off.aur_kernels.is_empty());
        assert_eq!(off.gate_message, None);

        // feature ON, paru/awk missing -> stderr message, no probe, no rows
        let gated = discover_aur(&AurDiscoveryInput {
            enabled: true,
            paru_available: false,
            awk_available: true,
            repo_kernels: repo_kernels.clone(),
            paru_output: String::new(),
        });
        assert!(!gated.probe_run);
        assert!(gated.aur_kernels.is_empty());
        assert_eq!(
            gated.gate_message.as_deref(),
            Some("Paru and/or AWK are not installed! Disabling AUR kernels support")
        );

        // feature ON, no repo kernels -> the `!kernels.empty()` gate: no probe
        let empty = discover_aur(&AurDiscoveryInput {
            enabled: true,
            paru_available: true,
            awk_available: true,
            repo_kernels: vec![],
            paru_output: "linux-cachyos-zen-headers\n".into(),
        });
        assert!(!empty.probe_run);
        assert!(empty.aur_kernels.is_empty());

        // feature ON, kernels present -> rows parsed, deduped, stripped
        // (the paru pipeline's awk '{print $2}' already emitted bare header
        // names — the probe output is the awk result)
        let on = discover_aur(&AurDiscoveryInput {
            enabled: true,
            paru_available: true,
            awk_available: true,
            repo_kernels: repo_kernels.clone(),
            paru_output: concat!(
                "linux-cachyos-zen-headers\n",
                "linux-cachyos\n", // dedup vs repo (name match)
                "linux-cachyos-rc-headers\n",
                "linux-cachyos-zen-headers\n", // duplicate AUR row
            )
            .into(),
        });
        assert!(on.probe_run);
        assert_eq!(on.gate_message, None);
        let names: Vec<&str> = on.aur_kernels.iter().map(|k| k.name.as_str()).collect();
        assert_eq!(names, vec!["linux-cachyos-zen", "linux-cachyos-rc"]);
        let zen = &on.aur_kernels[0];
        assert_eq!(zen.repo, "aur");
        assert_eq!(zen.headers, "linux-cachyos-zen-headers");
        assert_eq!(zen.version, "unknown-version");
        assert_eq!(zen.raw, "aur/linux-cachyos-zen");
    }

    #[test]
    fn discover_aur_strips_all_headers_occurrences() {
        // replace_all removes EVERY "-headers" (a header whose NAME also
        // contains "-headers-" mid-string, e.g. linux-cachyos-headers-dev)
        let repo = vec![DiscoveredKernel {
            repo: "cachyos".into(),
            name: "linux-cachyos".into(),
            headers: "linux-cachyos-headers".into(),
            version: "1.0-1".into(),
            companions: Default::default(),
            raw: "cachyos/linux-cachyos".into(),
        }];
        let res = discover_aur(&AurDiscoveryInput {
            enabled: true,
            paru_available: true,
            awk_available: true,
            repo_kernels: repo,
            paru_output: "linux-cachyos-headers-dev\n".into(),
        });
        assert!(res.probe_run);
        assert_eq!(res.aur_kernels.len(), 1);
        assert_eq!(res.aur_kernels[0].name, "linux-cachyos-dev");
    }
}
