//! `cachyos-kernel-manager-cancel` — candidate witness for the
//! `build-env/cancellation` court.
//!
//! Reads the SAME corpus schema as `tools/cancel-oracle-ref` (a sequence of
//! Configure-window user actions: execute/cancel/close) and renders the
//! candidate's REAL lifecycle model (exec crate, conf-window.cpp:688-701):
//! the `m_running` guard (a second Execute while running is a no-op), the
//! unconditional close (default closeEvent), and the m_running transitions.
//!
//! Usage: cachyos-kernel-manager-cancel parse <corpus.json>

use cachyos_kernel_manager_exec::{configure_trace, ConfigureAction};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    actions: Vec<ConfigureAction>,
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
            eprintln!("usage: cachyos-kernel-manager-cancel parse <corpus.json>");
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
    let (trace, final_running) = configure_trace(&corpus.actions);
    let trace_json: Vec<serde_json::Value> = trace
        .iter()
        .map(|e| json!({ "action": serde_json::to_value(e.action).unwrap(), "outcome": e.outcome }))
        .collect();
    let payload = json!({
        "schema": "cachyos-km-configure-trace-v1",
        "trace": trace_json,
        "final_running": final_running,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
