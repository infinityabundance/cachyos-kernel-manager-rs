//! `cachyos-kernel-manager-installcmd` — renders the candidate's MODELED
//! install command for the `boot/system-boot-after-install` court
//! (Phase 11): `pacman -S --needed <pkgs>` from the courted
//! `pacman_install_argv` (exec crate) for the kernel the court installs.
//!
//! The ORACLE side uses the frozen source's literal command; the candidate
//! side executes this MODEL-RENDERED string — the court witnesses at
//! runtime that they are one and the same.
//!
//! Usage: cachyos-kernel-manager-installcmd <pkg> [<pkg>...]

use cachyos_kernel_manager_exec::pacman_install_argv;

fn main() {
    let packages: Vec<String> = std::env::args().skip(1).collect();
    println!("{}", pacman_install_argv(&packages, true).join(" "));
}
