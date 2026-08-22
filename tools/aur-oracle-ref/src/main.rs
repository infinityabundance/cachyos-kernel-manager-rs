//! Reference harness reproducing the ORACLE's AUR support byte-for-byte
//! (`oracle/upstream/src/kernel.cpp` + `aur_kernel.cpp` +
//! `string_utils.hpp`, revision `6b4a373e`):
//!
//! Discovery (`kernel.cpp:253-283`, inside `#ifdef ENABLE_AUR_KERNELS`):
//! - `is_paru_installed` starts `true`; if EITHER `/sbin/paru` or `/sbin/awk`
//!   is missing: stderr `"Paru and/or AWK are not installed! Disabling AUR
//!   kernels support\n"` and `is_paru_installed = false`;
//! - the probe `paru --aur -Sl | grep ' linux[^ ]*-headers' | awk '{print $2}'`
//!   runs only when `!kernels.empty() && is_paru_installed`;
//! - each probe line (split on `\n`, empty segments dropped —
//!   `make_multiline`) has ALL `-headers` occurrences removed
//!   (`replace_all`); names already present (by name) are skipped; the row is
//!   appended with `repo="aur"`, `version="unknown-version"`,
//!   `raw="aur/<name>"`.
//!
//! Install (`kernel.cpp:89-95`): AUR kernels bypass `Kernel::install`'s
//! pacman expansion — the name is pushed to `g_aur_kernel_install_list`.
//!
//! Commit (`kernel.cpp:288-304` + `aur_kernel.cpp:42-55`): AUR installs
//! FIRST — per name, skip names containing the substring `"headers"`
//! (`std::ranges::search`), git-refresh `~/.cache/cachyos-km/aur_pkgbuilds/
//! <name>` from `https://aur.archlinux.org/<name>.git`, then `makepkg -sicf
//! --cleanbuild --skipchecksums` (`runCmdTerminal`, NOT escalated); then
//! `pacman -S --needed <install list>`; then `pacman -Rsn <remove list>`.
//!
//! Input: the shared corpus schema (`cachyos-km-aur-corpus-v1`). Output:
//! the model JSON (`cachyos-km-aur-model-v1`); the gate message goes to
//! stderr byte-for-byte.
//!
//! Usage: aur-oracle-ref parse <corpus.json>
//! This tool is court evidence infrastructure, never shipped.

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

const GATE_MESSAGE: &str = "Paru and/or AWK are not installed! Disabling AUR kernels support";
const AUR_CACHE_DIR: &str = "~/.cache/cachyos-km/aur_pkgbuilds";

/// The shared corpus schema (both the oracle-ref and the candidate CLI read
/// the SAME file).
#[derive(Debug, Deserialize)]
struct Corpus {
    /// `ENABLE_AUR_KERNELS` (meson `aur_kernels`, default off — the shipped
    /// CMake oracle has it OFF).
    aur_enabled: bool,
    /// `fs::exists("/sbin/paru")`.
    paru_available: bool,
    /// `fs::exists("/sbin/awk")`.
    awk_available: bool,
    /// Repo kernel names discovered before the AUR block (`kernels`).
    /// Only the NAME participates in the AUR dedup (`kernel.cpp:268`).
    #[serde(default)]
    repo_kernel_names: Vec<String>,
    /// The `paru --aur -Sl | grep ... | awk '{print $2}'` stdout — bare
    /// header package names, one per line, one trailing `\n` already
    /// stripped by the `utils::exec` popen parity.
    #[serde(default)]
    paru_output: String,
    /// AUR kernel names in the order `Kernel::install` pushed them to
    /// `g_aur_kernel_install_list` (change-list order).
    #[serde(default)]
    aur_selections: Vec<String>,
    /// The already-expanded repo install package list (courted elsewhere;
    /// fixed input here).
    #[serde(default)]
    install_packages: Vec<String>,
    /// The already-expanded repo removal package list.
    #[serde(default)]
    remove_packages: Vec<String>,
}

/// `make_multiline` (`string_utils.hpp:66-72`): split on `\n`, drop empty
/// segments.
fn multiline(str: &str) -> Vec<String> {
    str.split('\n')
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// `replace_all(inout, what, "")` (`string_utils.hpp:48-56`) — every
/// occurrence.
fn remove_all(inout: &mut String, what: &str) {
    while let Some(pos) = inout.find(what) {
        inout.replace_range(pos..pos + what.len(), "");
    }
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
            eprintln!("usage: aur-oracle-ref parse <corpus.json>");
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

    // ---------------------------------------------------------------
    // AUR discovery (kernel.cpp:253-283)
    // ---------------------------------------------------------------
    let mut probe_run = false;
    let mut aur_rows: Vec<serde_json::Value> = Vec::new();
    if corpus.aur_enabled {
        let mut is_paru_installed = true;
        if !corpus.paru_available || !corpus.awk_available {
            eprintln!("{GATE_MESSAGE}");
            is_paru_installed = false;
        }
        if !corpus.repo_kernel_names.is_empty() && is_paru_installed {
            probe_run = true;
            let mut seen: Vec<String> = corpus.repo_kernel_names.clone();
            for header in multiline(&corpus.paru_output) {
                let mut name = header.clone();
                remove_all(&mut name, "-headers");
                if seen.iter().any(|n| n == &name) {
                    continue;
                }
                seen.push(name.clone());
                aur_rows.push(json!({
                    "repo": "aur",
                    "name": name,
                    "headers": header,
                    "version": "unknown-version",
                    // AUR kernels never resolve companions (m_zfs_module,
                    // m_nvidia_module, m_nvidia_open_module stay nullptr —
                    // `Kernel::install` bypasses them, kernel.cpp:90-95).
                    "companions": {"zfs": null, "nvidia": null, "nvidia_open": null},
                    "raw": format!("aur/{name}"),
                }));
            }
        }
    }

    // ---------------------------------------------------------------
    // Commit (kernel.cpp:288-304 + aur_kernel.cpp:42-55)
    // ---------------------------------------------------------------
    let mut commit: Vec<serde_json::Value> = Vec::new();
    for name in &corpus.aur_selections {
        // std::ranges::search(kernel_name, "headers"sv) — substring match
        if name.contains("headers") {
            continue;
        }
        commit.push(json!({
            "kind": "git-refresh",
            "url": format!("https://aur.archlinux.org/{name}.git"),
            "dir": format!("{AUR_CACHE_DIR}/{name}"),
        }));
        commit.push(json!({
            "kind": "build-aur",
            "argv": ["makepkg", "-sicf", "--cleanbuild", "--skipchecksums"],
        }));
    }
    if !corpus.install_packages.is_empty() {
        let mut argv = vec![
            "pacman".to_string(),
            "-S".to_string(),
            "--needed".to_string(),
        ];
        argv.extend(corpus.install_packages.iter().cloned());
        commit.push(json!({ "kind": "install-repo", "argv": argv }));
    }
    if !corpus.remove_packages.is_empty() {
        let mut argv = vec!["pacman".to_string(), "-Rsn".to_string()];
        argv.extend(corpus.remove_packages.iter().cloned());
        commit.push(json!({ "kind": "remove-repo", "argv": argv }));
    }

    let payload = json!({
        "schema": "cachyos-km-aur-model-v1",
        "probe_run": probe_run,
        "aur_rows": aur_rows,
        "commit": commit,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
