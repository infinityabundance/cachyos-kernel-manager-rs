//! Candidate config CLI — differential config-roundtrip court tool.
//!
//! Usage: cachyos-kernel-manager-config parse <file>
//!
//! Parses a config file with the candidate's `KernelManagerConfig` (same
//! serde attributes and field order as the oracle's `config-option-lib`)
//! and prints the canonical re-serialization. Exit 0 on parse success,
//! 1 on parse error — mirroring the oracle's `parse_config_file` contract.
//!
//! The oracle side of the court is `tools/config-oracle-ref` (the upstream
//! struct + toml 1.1, the oracle's actual dependency versions). The
//! comparator byte-compares the canonical outputs.

use cachyos_kernel_manager_config::KernelManagerConfig;
use std::process::ExitCode;

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
            eprintln!("usage: cachyos-kernel-manager-config parse <file>");
            return ExitCode::from(2);
        }
    };
    match KernelManagerConfig::parse(&content) {
        Ok(cfg) => match cfg.to_toml_string() {
            Ok(s) => {
                print!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
