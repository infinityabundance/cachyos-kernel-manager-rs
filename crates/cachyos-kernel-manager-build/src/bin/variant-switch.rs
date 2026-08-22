//! `cachyos-kernel-manager-variant-switch` — candidate witness for the
//! `option-transitions/variant-switch` court.
//!
//! Reads the SAME sequence schema as `tools/options-oracle-ref`
//! (`{"switches": ["lts", ...]}`), applies the candidate's stateful
//! [`VariantSwitchState`] model, and prints the state after each switch in
//! the identical JSON form.
//!
//! Usage: cachyos-kernel-manager-variant-switch parse <sequence.json>

use cachyos_kernel_manager_core::options::{KernelVariant, VariantSwitchState};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Sequence {
    switches: Vec<String>,
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
            eprintln!("usage: cachyos-kernel-manager-variant-switch parse <sequence.json>");
            return ExitCode::from(2);
        }
    };
    let seq: Sequence = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut state = VariantSwitchState::default();
    let mut out = vec![state_to_json(&state, "initial")];
    for kernel in &seq.switches {
        let variant = match variant_by_id(kernel) {
            Some(v) => v,
            None => {
                eprintln!("unknown variant: {kernel:?}");
                return ExitCode::FAILURE;
            }
        };
        state.switch_to(variant);
        out.push(state_to_json(&state, kernel));
    }
    println!("{}", serde_json::to_string_pretty(&out).expect("serialize"));
    ExitCode::SUCCESS
}

fn variant_by_id(id: &str) -> Option<KernelVariant> {
    KernelVariant::ALL.iter().copied().find(|v| v.id() == id)
}

fn state_to_json(state: &VariantSwitchState, variant: &str) -> serde_json::Value {
    json!({
        "variant": variant,
        "lto_items": state.lto_items.iter().map(|l| l.value()).collect::<Vec<_>>(),
        "lto_selected": state.lto_selected.value(),
        "preempt_items": state.preempt_items.iter().map(|p| p.value()).collect::<Vec<_>>(),
        "preempt_selected": state.preempt_selected.value(),
        "hz_selected": state.hz_selected.value(),
        "cachy_config_checked": state.cachy_config_checked,
        "zfs_checked": state.zfs_checked,
        "zfs_enabled": state.zfs_enabled,
    })
}
