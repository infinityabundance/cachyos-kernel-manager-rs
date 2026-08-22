//! Reference harness reproducing the ORACLE's build-flow decisions
//! (`conf-window.cpp:696-735` on_execute, `378-405` finished_proc,
//! `aur_kernel.cpp:53`, `utils.cpp:122-135`, revision `6b4a373e`)
//! byte-for-byte:
//!
//! - `cpusched_path` = get_kernel_name_path(variant) (conf-window.cpp:124-148)
//! - `working_path` = `<fs::current_path()>/<cpusched_path>` (on_execute:730-731)
//! - repo build command = `makepkg -scf --cleanbuild --skipchecksums && touch .done-status`
//! - terminal-helper argv = `terminal-helper <cmd>; read -p 'Press enter to exit'`
//!   (run_cmd_async appends the pause, conf-window.cpp:361-376) — NO `-s`
//! - done-status path = `<working_path>/.done-status` (finished_proc:384)
//! - AUR build command = `makepkg -sicf --cleanbuild --skipchecksums` (aur_kernel.cpp:53)
//! - artifact install = `sudo pacman -U <globs joined by ' '>` (finished_proc:394-396)
//!
//! Input: `{"variant": "cachyos", "cwd": "/tmp", "globs": ["linux-cachyos-6.14.1-3-*.pkg.tar.zst"]}`
//! Output: the plan JSON. This tool is court evidence infrastructure.
//!
//! Usage: buildflow-oracle-ref parse <corpus.json>

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

const MAKEPKG_REPO_CMD: &str = "makepkg -scf --cleanbuild --skipchecksums && touch .done-status";
const MAKEPKG_AUR_CMD: &str = "makepkg -sicf --cleanbuild --skipchecksums";
const PRESS_ENTER_SUFFIX: &str = "; read -p 'Press enter to exit'";
const TERMINAL_HELPER: &str = "/usr/lib/cachyos-kernel-manager/terminal-helper";

#[derive(Debug, Deserialize)]
struct Corpus {
    variant: String,
    cwd: String,
    globs: Vec<String>,
}

/// get_kernel_name_path (`conf-window.cpp:124-148`); fallback linux-cachyos.
fn kernel_name_path(kernel_name: &str) -> &'static str {
    match kernel_name {
        "cachyos" => "linux-cachyos",
        "bmq" => "linux-cachyos-bmq",
        "bore" => "linux-cachyos-bore",
        "hardened" => "linux-cachyos-hardened",
        "lts" => "linux-cachyos-lts",
        "rc" => "linux-cachyos-rc",
        "rt" => "linux-cachyos-rt-bore",
        "eevdf" => "linux-cachyos-eevdf",
        "deckify" => "linux-cachyos-deckify",
        "server" => "linux-cachyos-server",
        _ => "linux-cachyos",
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
            eprintln!("usage: buildflow-oracle-ref parse <corpus.json>");
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
    let cpusched_path = kernel_name_path(&corpus.variant);
    let working_path = format!("{}/{}", corpus.cwd, cpusched_path);
    let terminal_argv = vec![
        TERMINAL_HELPER.to_string(),
        format!("{MAKEPKG_REPO_CMD}{PRESS_ENTER_SUFFIX}"),
    ];
    let payload = json!({
        "variant": corpus.variant,
        "cpusched_path": cpusched_path,
        "working_path": working_path,
        "build_command": MAKEPKG_REPO_CMD,
        "terminal_argv": terminal_argv,
        "done_status": format!("{working_path}/.done-status"),
        "aur_build_command": MAKEPKG_AUR_CMD,
        "artifact_install_command": format!("sudo pacman -U {}", corpus.globs.join(" ")),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
