//! cachyos-kernel-manager — application entry point.
//!
//! Phase status: this binary currently performs the foundation diagnostics
//! (oracle freeze verification + identity report). The Iced GUI replaces the
//! interactive surface in Phase 8 (docs/ARCHITECTURE.md); the single-instance
//! lock and org/app identity semantics from `oracle/upstream/src/main.cpp`
//! are reconstructed in the platform crate.

use cachyos_kernel_manager_config::KernelManagerConfig;
use cachyos_kernel_manager_core::options::BuildOptions;
use cachyos_kernel_manager_oracle::UpstreamLock;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("cachyos-kernel-manager {VERSION}");
        return;
    }

    // Locate the repository root relative to the executable for diagnostics
    // (during development we run from the workspace; the packaged binary in
    // Phase 10 embeds the lock instead).
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_path = repo_root.join("oracle/UPSTREAM.lock");

    println!("CachyOS Kernel Manager (native Rust reimplementation)");
    println!("version: {VERSION}");
    match UpstreamLock::load(&lock_path) {
        Ok(lock) => {
            println!(
                "oracle: {} v{}",
                lock.oracle.repository, lock.oracle.version
            );
            println!(
                "oracle commit: {} (tree {})",
                lock.oracle.commit, lock.oracle.tree
            );
            println!("binary identity: {}", lock.identity.binary);
            println!("polkit action: {}", lock.identity.polkit_action);
            match lock.verify_archive(&repo_root) {
                Ok(true) => println!("oracle archive hash: OK"),
                Ok(false) => {
                    eprintln!(
                        "WARNING: oracle archive hash MISMATCH — the freeze is not reproducible"
                    );
                }
                Err(e) => eprintln!("WARNING: cannot verify oracle archive: {e}"),
            }
        }
        Err(e) => eprintln!("WARNING: cannot load oracle lock: {e}"),
    }

    // Self-check the pure domain model (build option env rendering).
    let opts = BuildOptions::default();
    println!(
        "default build env (first line): {}",
        opts.env_pairs()
            .first()
            .map(|(v, val)| format!("{v}={val}"))
            .unwrap_or_default()
    );
    println!(
        "default config serializes: {}",
        KernelManagerConfig::default()
            .to_toml_string()
            .map(|s| s.lines().next().unwrap_or("").to_string())
            .unwrap_or_default()
    );
    println!("foundation OK — GUI lands in Phase 8; see docs/ARCHITECTURE.md");
}
