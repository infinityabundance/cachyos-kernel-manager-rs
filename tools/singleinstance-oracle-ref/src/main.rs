//! Reference re-declaration of the ORACLE's single-instance lock decision
//! for the `single-instance/stale-lock` court (`cachyos-km-single-instance-v1`).
//!
//! Reproduced from `main.cpp:45-56` (`IsInstanceAlreadyRunning`):
//!
//! ```cpp
//! if (!memoryLock.create(1)) {
//!     memoryLock.attach();
//!     memoryLock.detach();
//!     if (!memoryLock.create(1)) {
//!         return true; // already running
//!     }
//! }
//! ```
//!
//! with the QSharedMemory unix backend semantics (qsharedmemory_unix.cpp):
//! `create` fails when the segment exists; the segment is marked IPC_RMID on
//! the creator's attach, so it is destroyed when the LAST attachment ends
//! (a crashed holder therefore releases it); `attach` succeeds iff the
//! segment exists; `detach` succeeds after a successful attach.
//!
//! This tool is court evidence infrastructure, never shipped.
//!
//! Usage: singleinstance-oracle-ref parse <corpus.json>

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

/// `IsInstanceAlreadyRunning` (`main.cpp:45-56`).
fn decide(create1_ok: bool, attach_ok: bool, detach_ok: bool, create2_ok: bool) -> bool {
    if create1_ok {
        return false; // not already running
    }
    if attach_ok {
        if !detach_ok {
            return true;
        }
        return !create2_ok;
    }
    // attach failed: the segment vanished between the create failure and
    // the attach — only the retry decides
    !create2_ok
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: singleinstance-oracle-ref parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: singleinstance-oracle-ref parse <corpus.json>");
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
    let already_running = decide(
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
        "decision": if already_running { "already-running" } else { "proceed" },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
