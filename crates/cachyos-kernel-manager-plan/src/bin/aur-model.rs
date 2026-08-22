//! `cachyos-kernel-manager-aur-model` — candidate witness for the
//! `aur/enablement-matrix` court.
//!
//! Reads the SAME corpus schema as `tools/aur-oracle-ref`
//! (`cachyos-km-aur-corpus-v1`: the feature flag, paru/awk availability,
//! repo kernel names, the paru probe output, the AUR selections and the
//! pre-expanded repo install/remove lists) and renders the candidate's REAL
//! AUR model:
//!
//! - `discover_aur` (plan crate) — the oracle's AUR discovery block
//!   (`kernel.cpp:253-283`): gating, the paru probe, row construction,
//!   `-headers` stripping, dedup;
//! - `expand_aur_install` + `commit_commands` (plan crate) — the oracle's
//!   commit ordering (`kernel.cpp:288-304`, `aur_kernel.cpp:42-55`): the
//!   git-refresh + `makepkg -sicf` pair per AUR kernel (headers-skip),
//!   FIRST, then the repo install, then the repo removal.
//!
//! The gate message is written to stderr byte-for-byte (the oracle's
//! `fmt::print(stderr, ...)`); the court compares stderr files exactly.
//!
//! Usage: cachyos-kernel-manager-aur-model parse <corpus.json>

use cachyos_kernel_manager_core::discovery::DiscoveredKernel;
use cachyos_kernel_manager_exec::CommandPlan;
use cachyos_kernel_manager_plan::{
    commit_commands, discover_aur, expand_aur_install, AurDiscoveryInput, PackageAction, Reason,
    TransactionPlan,
};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    aur_enabled: bool,
    paru_available: bool,
    awk_available: bool,
    #[serde(default)]
    repo_kernel_names: Vec<String>,
    #[serde(default)]
    paru_output: String,
    #[serde(default)]
    aur_selections: Vec<String>,
    #[serde(default)]
    install_packages: Vec<String>,
    #[serde(default)]
    remove_packages: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let content = match args.as_slice() {
        [cmd, path] if cmd == "parse" => match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: cachyos-kernel-manager-aur-model parse <corpus.json>");
            return ExitCode::from(2);
        }
    };
    let corpus: Corpus = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // Discovery inputs: only the NAME participates in the AUR dedup
    // (`kernel.cpp:268` — `find_if(kernels, m_name == aur_kernel)`), so the
    // corpus carries names and the row fields are filled by the model.
    let repo_kernels: Vec<DiscoveredKernel> = corpus
        .repo_kernel_names
        .iter()
        .map(|name| DiscoveredKernel {
            repo: "cachyos".to_string(),
            name: name.clone(),
            headers: format!("{name}-headers"),
            version: "0".to_string(),
            companions: Default::default(),
            raw: format!("cachyos/{name}"),
        })
        .collect();

    let discovery = discover_aur(&AurDiscoveryInput {
        enabled: corpus.aur_enabled,
        paru_available: corpus.paru_available,
        awk_available: corpus.awk_available,
        repo_kernels,
        paru_output: corpus.paru_output.clone(),
    });
    if let Some(msg) = &discovery.gate_message {
        eprintln!("{msg}");
    }

    let aur_rows: Vec<serde_json::Value> = discovery
        .aur_kernels
        .iter()
        .map(|k| {
            json!({
                "repo": k.repo,
                "name": k.name,
                "headers": k.headers,
                "version": k.version,
                "companions": {
                    "zfs": k.companions.zfs,
                    "nvidia": k.companions.nvidia,
                    "nvidia_open": k.companions.nvidia_open,
                },
                "raw": k.raw,
            })
        })
        .collect();

    let mut plan = TransactionPlan {
        install: corpus
            .install_packages
            .iter()
            .map(|p| PackageAction {
                package: p.clone(),
                reason: Reason::SelectedKernel,
            })
            .collect(),
        remove: corpus
            .remove_packages
            .iter()
            .map(|p| PackageAction {
                package: p.clone(),
                reason: Reason::SelectedKernel,
            })
            .collect(),
        aur_install: Vec::new(),
        aur_enabled: corpus.aur_enabled,
        warnings: Vec::new(),
    };
    for name in &corpus.aur_selections {
        expand_aur_install(&mut plan, name);
    }
    let commit: Vec<serde_json::Value> =
        commit_commands(&plan).iter().map(render_command).collect();

    let payload = json!({
        "schema": "cachyos-km-aur-model-v1",
        "probe_run": discovery.probe_run,
        "aur_rows": aur_rows,
        "commit": commit,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}

/// Render a [`CommandPlan`] in the same shape as the oracle-ref: the commit
/// sequence as `kind` + fields/argv.
fn render_command(command: &CommandPlan) -> serde_json::Value {
    match command {
        CommandPlan::GitRefresh { url, dir } => {
            json!({ "kind": "git-refresh", "url": url, "dir": dir })
        }
        CommandPlan::BuildAurPackage => json!({
            "kind": "build-aur",
            "argv": ["makepkg", "-sicf", "--cleanbuild", "--skipchecksums"],
        }),
        CommandPlan::InstallRepoPackages { packages, needed } => json!({
            "kind": "install-repo",
            "argv": cachyos_kernel_manager_exec::pacman_install_argv(packages, *needed),
        }),
        CommandPlan::RemovePackages { packages } => json!({
            "kind": "remove-repo",
            "argv": cachyos_kernel_manager_exec::pacman_remove_argv(packages),
        }),
        // Never emitted by commit_commands; rendered for schema totality.
        CommandPlan::BuildKernelPackage => json!({
            "kind": "build-kernel",
            "argv": ["makepkg", "-scf", "--cleanbuild", "--skipchecksums"],
        }),
        CommandPlan::InstallLocalPackages { globs } => json!({
            "kind": "install-local",
            "argv": ["sudo", "pacman", "-U", globs],
        }),
    }
}
