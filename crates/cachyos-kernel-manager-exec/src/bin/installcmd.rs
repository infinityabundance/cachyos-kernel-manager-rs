//! `cachyos-kernel-manager-installcmd` — renders the candidate's MODELED
//! transaction commands for the Phase 11 boot courts: `pacman -S --needed
//! <pkgs>` (install, `pacman_install_argv`) and `pacman -Rsn <pkgs>`
//! (remove, `pacman_remove_argv`) from the exec crate.
//!
//! The ORACLE side uses the frozen source's literal commands; the candidate
//! side executes these MODEL-RENDERED strings — the courts witness at
//! runtime that they are one and the same.
//!
//! Usage: cachyos-kernel-manager-installcmd install <pkg> [<pkg>...]
//!        cachyos-kernel-manager-installcmd remove  <pkg> [<pkg>...]

use cachyos_kernel_manager_exec::{pacman_install_argv, pacman_remove_argv};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, packages) = match args.as_slice() {
        [mode, rest @ ..] => (mode.as_str(), rest.to_vec()),
        _ => {
            eprintln!("usage: cachyos-kernel-manager-installcmd install|remove <pkg>...");
            std::process::exit(2);
        }
    };
    match mode {
        "install" => println!("{}", pacman_install_argv(&packages, true).join(" ")),
        "remove" => println!("{}", pacman_remove_argv(&packages).join(" ")),
        _ => {
            eprintln!("usage: cachyos-kernel-manager-installcmd install|remove <pkg>...");
            std::process::exit(2);
        }
    }
}
