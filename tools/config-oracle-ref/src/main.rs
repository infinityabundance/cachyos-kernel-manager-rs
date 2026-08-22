//! Reference harness reproducing the ORACLE's config serialization exactly.
//!
//! Source: `oracle/upstream/config-option-lib/src/lib.rs` (revision
//! `6b4a373e`) — the upstream's config TOML layer is itself Rust
//! (serde + toml 1.1 + `#[serde(default)]`). This tool re-declares the
//! upstream struct verbatim (same fields, same order, same serde
//! attributes) and uses the same `toml` major version, so its output IS
//! the oracle's output.
//!
//! Usage: config-oracle-ref parse <file>   -> canonical re-serialization
//!        config-oracle-ref stdin          -> canonical re-serialization
//! The exit status is 0 on successful parse, 1 on parse error (the oracle
//! surfaces parse errors as "Failed to load config options from file").
//!
//! This tool is court evidence infrastructure, never shipped.

use serde::{Deserialize, Serialize};

/// Verbatim copy of the oracle's `Config` (config-option-lib/src/lib.rs).
#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hardly_check: bool,
    pub per_gov_check: bool,
    pub tcp_bbr3_check: bool,

    pub cachy_config_check: bool,
    pub nconfig_check: bool,
    pub xconfig_check: bool,
    pub localmodcfg_check: bool,
    pub use_current_check: bool,
    pub builtin_zfs_check: bool,
    pub builtin_nvidia_open_check: bool,
    pub build_debug_check: bool,

    pub hz_ticks_combo: String,
    pub tickrate_combo: String,
    pub preempt_combo: String,
    pub hugepage_combo: String,
    pub lto_combo: String,

    pub cpu_opt_combo: String,
    pub custom_name_edit: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let content = match args.as_slice() {
        [] => {
            let mut s = String::new();
            use std::io::Read;
            std::io::stdin().read_to_string(&mut s).expect("read stdin");
            s
        }
        [cmd, path] if cmd == "parse" => std::fs::read_to_string(path).expect("read file"),
        _ => {
            eprintln!("usage: config-oracle-ref [parse <file>]");
            std::process::exit(2);
        }
    };
    match toml::from_str::<Config>(&content) {
        Ok(cfg) => {
            print!("{}", toml::to_string(&cfg).expect("serialize"));
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
