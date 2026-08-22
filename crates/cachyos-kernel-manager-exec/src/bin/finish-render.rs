//! `cachyos-kernel-manager-finish` — candidate witness for the
//! `build-env/failure-lifecycle` court.
//!
//! Reads the SAME corpus schema as `tools/finish-oracle-ref` (a sequence of
//! async-process completions: `.done-status` existence, exit code, the
//! install-dialog answer, artifact globs) and renders the candidate's REAL
//! `finished_proc` model (exec crate, conf-window.cpp:378-405): the
//! stdout/stderr lines, the re-entrant install command, the `.done-status`
//! removal, and the m_running transitions.
//!
//! Usage: cachyos-kernel-manager-finish parse <corpus.json>

use cachyos_kernel_manager_exec::{finished_proc, FinishEvent};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CorpusEvent {
    done_status_exists: bool,
    exit_code: i32,
    user_choice: Option<bool>,
    #[serde(default)]
    globs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    events: Vec<CorpusEvent>,
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
            eprintln!("usage: cachyos-kernel-manager-finish parse <corpus.json>");
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
    let events: Vec<serde_json::Value> = corpus
        .events
        .iter()
        .map(|e| {
            let outcome = finished_proc(&FinishEvent {
                done_status_exists: e.done_status_exists,
                exit_code: e.exit_code,
                user_choice: e.user_choice,
                globs: e.globs.clone(),
            });
            json!({
                "stdout": outcome.stdout,
                "stderr": outcome.stderr,
                "next_command": outcome.next_command,
                "removes_done_status": outcome.removes_done_status,
                "running_after": outcome.running_after,
            })
        })
        .collect();
    let payload = json!({ "schema": "cachyos-km-finish-v1", "events": events });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
