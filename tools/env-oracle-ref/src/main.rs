//! Reference harness reproducing the ORACLE's build-env rendering
//! (`get_all_set_values`, conf-window.cpp:421-451) byte-for-byte.
//!
//! Source: `oracle/upstream/src/conf-window.cpp` + `compile_options.json`
//! (revision `6b4a373e`). The oracle renders one `var=value\n` line per
//! option: 11 checkboxes always emit yes/no (binding order
//! conf-window.cpp:164-176, var names from compile_options.json's
//! `option_map`), then the combos `_HZ_ticks`/`_tickrate`/`_preempt`/
//! `_hugepage`/`_use_llvm_lto`, then `_processor_opt` only when != manual,
//! then the `_use_lto_suffix=n` workaround when lto != none and the custom
//! name != `$pkgbase`.
//!
//! The corpus schema mirrors the UI state (values, not indexes — the UI
//! combo holds exactly these values). Values outside the combo lists cannot
//! exist in the UI and are rejected (exit 1), mirroring the candidate's
//! enum parse.
//!
//! Usage: env-oracle-ref parse <corpus.json>   -> env string on stdout
//! This tool is court evidence infrastructure, never shipped.

use serde::Deserialize;
use std::collections::HashMap;
use std::process::ExitCode;

/// The oracle's checkbox binding order (`conf-window.cpp:164-176`).
const CHECKBOX_BINDINGS: [&str; 11] = [
    "hardly",
    "per_gov",
    "tcp_bbr3",
    "cachy_config",
    "nconfig",
    "xconfig",
    "localmodcfg",
    "use_current",
    "builtin_zfs",
    "builtin_nvidia_open",
    "build_debug",
];

/// `detail::option_map` (generated verbatim from compile_options.json).
fn option_map() -> HashMap<&'static str, &'static str> {
    [
        ("cachy_config", "_cachy_config"),
        ("nconfig", "_makenconfig"),
        ("xconfig", "_makexconfig"),
        ("localmodcfg", "_localmodcfg"),
        ("use_current", "_use_current"),
        ("hardly", "_cc_harder"),
        ("per_gov", "_per_gov"),
        ("tcp_bbr3", "_tcp_bbr3"),
        ("HZ_ticks", "_HZ_ticks"),
        ("tickrate", "_tickrate"),
        ("preempt", "_preempt"),
        ("hugepage", "_hugepage"),
        ("cpu_opt", "_processor_opt"),
        ("lto", "_use_llvm_lto"),
        ("builtin_zfs", "_build_zfs"),
        ("builtin_nvidia_open", "_build_nvidia_open"),
        ("build_debug", "_build_debug"),
    ]
    .into_iter()
    .collect()
}

/// Combo value lists (`conf-window.cpp:104-109`).
const HZ_TICKS: [&str; 7] = ["1000", "750", "600", "500", "300", "250", "100"];
const TICKLESS: [&str; 3] = ["full", "idle", "periodic"];
const PREEMPT: [&str; 4] = ["full", "lazy", "voluntary", "none"];
const LTO: [&str; 4] = ["none", "full", "thin", "thin-dist"];
const HUGEPAGE: [&str; 2] = ["always", "madvise"];
const CPU_OPT: [&str; 7] = [
    "manual",
    "native",
    "generic_v1",
    "generic_v2",
    "generic_v3",
    "generic_v4",
    "zen4",
];

/// The corpus schema: the UI state the env string is rendered from.
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

/// `convert_to_var_assign(option, value)` — `option_map.at(option)=value\n`.
fn var_assign(option: &str, value: &str) -> String {
    let map = option_map();
    format!("{}={value}\n", map[option])
}

/// `convert_to_var_assign_empty_wrapped` — enabled -> "yes", else "no".
fn var_assign_wrapped(option: &str, enabled: bool) -> String {
    var_assign(option, if enabled { "yes" } else { "no" })
}

fn validate(value: &str, list: &[&str], what: &str) -> Result<(), String> {
    if list.contains(&value) {
        Ok(())
    } else {
        Err(format!("{what}: {value:?} is not a UI-possible value"))
    }
}

fn render(state: &UiState) -> Result<String, String> {
    let checkboxes = [
        state.hardly,
        state.per_gov,
        state.tcp_bbr3,
        state.cachy_config,
        state.nconfig,
        state.xconfig,
        state.localmodcfg,
        state.use_current,
        state.builtin_zfs,
        state.builtin_nvidia_open,
        state.build_debug,
    ];
    validate(&state.hz_ticks, &HZ_TICKS, "hz_ticks")?;
    validate(&state.tickrate, &TICKLESS, "tickrate")?;
    validate(&state.preempt, &PREEMPT, "preempt")?;
    validate(&state.hugepage, &HUGEPAGE, "hugepage")?;
    validate(&state.lto, &LTO, "lto")?;
    validate(&state.cpu_opt, &CPU_OPT, "cpu_opt")?;

    let mut out = String::new();
    for (binding, enabled) in CHECKBOX_BINDINGS.iter().zip(checkboxes) {
        out.push_str(&var_assign_wrapped(binding, enabled));
    }
    out.push_str(&var_assign("HZ_ticks", &state.hz_ticks));
    out.push_str(&var_assign("tickrate", &state.tickrate));
    out.push_str(&var_assign("preempt", &state.preempt));
    out.push_str(&var_assign("hugepage", &state.hugepage));
    out.push_str(&var_assign("lto", &state.lto));
    if state.cpu_opt != "manual" {
        out.push_str(&var_assign("cpu_opt", &state.cpu_opt));
    }
    // NOTE: workaround PKGBUILD incorrectly working with custom pkgname
    if state.lto != "none" && state.custom_name != "$pkgbase" {
        out.push_str("_use_lto_suffix=n\n");
    }
    Ok(out)
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
            eprintln!("usage: env-oracle-ref parse <corpus.json>");
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
    match render(&state) {
        Ok(s) => {
            print!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
