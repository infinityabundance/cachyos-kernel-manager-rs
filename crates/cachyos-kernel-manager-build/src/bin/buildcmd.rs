//! `cachyos-kernel-manager-buildcmd` — renders the candidate's MODELED
//! build commands for the `build-env/makepkg-runtime` court (gap-006): the
//! repo build command (`conf-window.cpp:734`) and the AUR build command
//! (`aur_kernel.cpp:53`), from the courted exec crate models
//! (`BuildFlowPlan::render` + `makepkg_aur_argv`).
//!
//! The ORACLE side of the court uses the frozen source's LITERAL command
//! strings; the CANDIDATE side executes these MODEL-RENDERED strings — the
//! court witnesses at runtime that they are one and the same.
//!
//! Usage: cachyos-kernel-manager-buildcmd

use cachyos_kernel_manager_core::options::KernelVariant;
use cachyos_kernel_manager_exec::{makepkg_aur_argv, BuildFlowPlan};

fn main() {
    let plan = BuildFlowPlan::render(KernelVariant::Cachyos, "/home/test/build-proj", &[]);
    println!("repo_build_command={}", plan.build_command);
    println!("aur_build_command={}", makepkg_aur_argv().join(" "));
}
