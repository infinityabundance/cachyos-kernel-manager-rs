//! `cachyos-kernel-manager-env` — candidate env-rendering witness for the
//! `build-env/env-rendering` court.
//!
//! Reads the SAME corpus schema as `tools/env-oracle-ref` (the UI state:
//! checkbox booleans, combo VALUES, custom name) and renders the candidate's
//! modeled env string (`BuildOptions::env_string`, which reproduces
//! `get_all_set_values` conf-window.cpp:421-451). Values outside the combo
//! lists cannot exist in the UI and fail the enum parse (exit 1), mirroring
//! the oracle reference's validation.
//!
//! Usage: cachyos-kernel-manager-env parse <corpus.json>

use cachyos_kernel_manager_core::options::{
    BuildOptions, CpuOptMode, HugepageMode, HzTick, KernelVariant, LtoMode, PreemptMode,
    TicklessMode,
};
use serde::Deserialize;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct UiState {
    hardly: bool,
    per_gov: bool,
    tcp_bbr3: bool,
    cachy_config: bool,
    nconfig: bool,
    xconfig: bool,
    localmodcfg: bool,
    use_current: bool,
    builtin_zfs: bool,
    builtin_nvidia_open: bool,
    build_debug: bool,
    hz_ticks: String,
    tickrate: String,
    preempt: String,
    hugepage: String,
    lto: String,
    cpu_opt: String,
    custom_name: String,
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
            eprintln!("usage: cachyos-kernel-manager-env parse <corpus.json>");
            return ExitCode::from(2);
        }
    };
    let state: UiState = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = match build_options(&state) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    print!("{}", opts.env_string());
    ExitCode::SUCCESS
}

fn build_options(state: &UiState) -> Result<BuildOptions, String> {
    let find_hz = |v: &str| HzTick::ALL.iter().find(|h| h.value() == v).copied();
    let find_tick = |v: &str| TicklessMode::ALL.iter().find(|t| t.value() == v).copied();
    let find_preempt = |v: &str| PreemptMode::ALL.iter().find(|p| p.value() == v).copied();
    let find_huge = |v: &str| HugepageMode::ALL.iter().find(|h| h.value() == v).copied();
    let find_lto = |v: &str| LtoMode::ALL.iter().find(|l| l.value() == v).copied();
    let find_cpu = |v: &str| CpuOptMode::ALL.iter().find(|c| c.value() == v).copied();

    let hz_ticks = find_hz(&state.hz_ticks)
        .ok_or_else(|| format!("hz_ticks: {:?} is not a UI-possible value", state.hz_ticks))?;
    let tickless = find_tick(&state.tickrate)
        .ok_or_else(|| format!("tickrate: {:?} is not a UI-possible value", state.tickrate))?;
    let preempt = find_preempt(&state.preempt)
        .ok_or_else(|| format!("preempt: {:?} is not a UI-possible value", state.preempt))?;
    let hugepage = find_huge(&state.hugepage)
        .ok_or_else(|| format!("hugepage: {:?} is not a UI-possible value", state.hugepage))?;
    let lto = find_lto(&state.lto)
        .ok_or_else(|| format!("lto: {:?} is not a UI-possible value", state.lto))?;
    let cpu_opt = find_cpu(&state.cpu_opt)
        .ok_or_else(|| format!("cpu_opt: {:?} is not a UI-possible value", state.cpu_opt))?;

    Ok(BuildOptions {
        variant: KernelVariant::Cachyos, // env rendering is variant-independent
        hardly: state.hardly,
        per_gov: state.per_gov,
        tcp_bbr3: state.tcp_bbr3,
        cachy_config: state.cachy_config,
        nconfig: state.nconfig,
        xconfig: state.xconfig,
        localmodcfg: state.localmodcfg,
        use_current: state.use_current,
        builtin_zfs: state.builtin_zfs,
        builtin_nvidia_open: state.builtin_nvidia_open,
        build_debug: state.build_debug,
        hz_ticks,
        tickless,
        preempt,
        hugepage,
        lto,
        cpu_opt,
        custom_name: state.custom_name.clone(),
    })
}
