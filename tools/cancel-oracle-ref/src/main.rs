//! Reference harness reproducing the ORACLE's Configure-window lifecycle
//! (`oracle/upstream/src/conf-window.cpp:549-550,688-701`, revision
//! `6b4a373e`) byte-for-byte:
//!
//! - `OK` → `on_execute`: `if (m_running) { return; }` — a second Execute
//!   while a build/install runs is a complete NO-OP (no command, no probe,
//!   m_running unchanged); otherwise `m_running = true` and the build
//!   starts;
//! - `Cancel` → `on_cancel` → `close()`; the WM close → `closeEvent` which
//!   calls `QWidget::closeEvent(event)` — accepted UNCONDITIONALLY (no
//!   confirmation, no blocking). The window (and its `QProcess m_cmd`
//!   member) is destroyed; the QProcess destructor terminates the in-flight
//!   child (terminal-helper/makepkg), which IS the oracle's cancellation
//!   semantics;
//! - after a close/cancel the window is gone: further actions are
//!   unreachable and emit nothing.
//!
//! Input: `{"actions": ["execute"|"cancel"|"close", ...]}`.
//! Output: `{"schema": "cachyos-km-configure-trace-v1", "trace":
//! [{"action": ..., "outcome": "start"|"ignored"|"closed"}, ...],
//! "final_running": bool}`.
//!
//! Usage: cancel-oracle-ref parse <corpus.json>
//! This tool is court evidence infrastructure, never shipped.

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    actions: Vec<String>,
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
            eprintln!("usage: cancel-oracle-ref parse <corpus.json>");
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
    let mut running = false;
    let mut trace: Vec<serde_json::Value> = Vec::new();
    for action in &corpus.actions {
        match action.as_str() {
            "execute" => {
                if running {
                    trace.push(json!({ "action": "execute", "outcome": "ignored" }));
                } else {
                    running = true;
                    trace.push(json!({ "action": "execute", "outcome": "start" }));
                }
            }
            "cancel" | "close" => {
                running = false;
                trace.push(json!({ "action": action, "outcome": "closed" }));
                break;
            }
            _ => {
                eprintln!("unknown action: {action:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    let payload = json!({
        "schema": "cachyos-km-configure-trace-v1",
        "trace": trace,
        "final_running": running,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
