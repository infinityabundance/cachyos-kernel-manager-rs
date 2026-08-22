//! `cachyos-kernel-manager-single-instance` — candidate witness for the
//! `single-instance/stale-lock` court. Reads the corpus (a lock-state
//! scenario: the four OS outcomes the oracle's `IsInstanceAlreadyRunning`
//! observes) and renders the candidate's REAL decision
//! (`cachyos_kernel_manager_platform::single_instance::decide`).
//!
//! Usage: cachyos-kernel-manager-single-instance parse <corpus.json>

use cachyos_kernel_manager_platform::single_instance::{decide, LockDecision};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    scenario: String,
    create1_ok: bool,
    attach_ok: bool,
    detach_ok: bool,
    create2_ok: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: cachyos-kernel-manager-single-instance parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: cachyos-kernel-manager-single-instance parse <corpus.json>");
        return ExitCode::from(2);
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let corpus: Corpus = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let decision = decide(
        corpus.create1_ok,
        corpus.attach_ok,
        corpus.detach_ok,
        corpus.create2_ok,
    );
    let payload = json!({
        "schema": "cachyos-km-single-instance-v1",
        "scenario": corpus.scenario,
        "create1_ok": corpus.create1_ok,
        "attach_ok": corpus.attach_ok,
        "detach_ok": corpus.detach_ok,
        "create2_ok": corpus.create2_ok,
        "decision": match decision {
            LockDecision::Proceed => "proceed",
            LockDecision::AlreadyRunning => "already-running",
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
