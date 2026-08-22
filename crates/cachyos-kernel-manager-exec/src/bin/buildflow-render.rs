//! `cachyos-kernel-manager-buildflow` — candidate witness for the
//! `build-env/lifecycle` court.
//!
//! Reads the SAME corpus schema as `tools/buildflow-oracle-ref`
//! (`{"variant": ..., "cwd": ..., "globs": [...]}`) and renders the
//! candidate's `BuildFlowPlan` in the identical JSON form.
//!
//! Usage: cachyos-kernel-manager-buildflow parse <corpus.json>

use cachyos_kernel_manager_core::options::KernelVariant;
use cachyos_kernel_manager_exec::BuildFlowPlan;
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    variant: String,
    cwd: String,
    globs: Vec<String>,
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
            eprintln!("usage: cachyos-kernel-manager-buildflow parse <corpus.json>");
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
    let variant = match KernelVariant::ALL
        .iter()
        .copied()
        .find(|v| v.id() == corpus.variant)
    {
        Some(v) => v,
        None => {
            eprintln!("unknown variant: {:?}", corpus.variant);
            return ExitCode::FAILURE;
        }
    };
    let plan = BuildFlowPlan::render(variant, &corpus.cwd, &corpus.globs);
    let payload = json!({
        "variant": plan.variant.id(),
        "cpusched_path": plan.cpusched_path,
        "working_path": plan.working_path,
        "build_command": plan.build_command,
        "terminal_argv": plan.terminal_argv,
        "done_status": plan.done_status,
        "aur_build_command": plan.aur_build_command,
        "artifact_install_command": plan.artifact_install_command,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
