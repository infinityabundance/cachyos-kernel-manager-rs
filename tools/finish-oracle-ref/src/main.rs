//! Reference harness reproducing the ORACLE's `finished_proc`
//! (`oracle/upstream/src/conf-window.cpp:378-405`, revision `6b4a373e`)
//! byte-for-byte:
//!
//! - `m_running = false` first;
//! - if `<m_build_conf_path>/.done-status` EXISTS: remove it, stdout
//!   `success`; ask `Do you want to install build packages?`; on Yes:
//!   stdout `pressed yes`, `pacman_cmd := sudo pacman -U <globs joined by
//!   ' '>` (the artifact-glob probe input), `m_running = true`, and the
//!   install runs through the SAME `run_cmd_async` — its OWN completion
//!   re-enters `finished_proc`, where the file is gone, so even a successful
//!   install prints `process failed with exit code: 0` to stderr;
//! - if the file is ABSENT: stderr `process failed with exit code: <n>\n`.
//!
//! The success decision keys on the FILE, never the exit code.
//!
//! Input: `{"events": [{"done_status_exists": bool, "exit_code": int,
//! "user_choice": "yes"|"no"|null, "globs": [...]}]}`.
//! Output: `{"schema": "cachyos-km-finish-v1", "events": [outcome...]}`.
//!
//! Usage: finish-oracle-ref parse <corpus.json>
//! This tool is court evidence infrastructure, never shipped.

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
            eprintln!("usage: finish-oracle-ref parse <corpus.json>");
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
            let mut stdout: Vec<String> = Vec::new();
            let mut stderr: Vec<String> = Vec::new();
            let mut next_command: Option<String> = None;
            let mut removes_done_status = false;
            let mut running_after = false;
            if e.done_status_exists {
                removes_done_status = true;
                stdout.push("success".to_string());
                if e.user_choice == Some(true) {
                    stdout.push("pressed yes".to_string());
                    let cmd = format!("sudo pacman -U {}", e.globs.join(" "));
                    stdout.push(format!("pacman_cmd := {cmd}"));
                    next_command = Some(cmd);
                    running_after = true;
                }
            } else {
                stderr.push(format!("process failed with exit code: {}\n", e.exit_code));
            }
            json!({
                "stdout": stdout,
                "stderr": stderr,
                "next_command": next_command,
                "removes_done_status": removes_done_status,
                "running_after": running_after,
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
