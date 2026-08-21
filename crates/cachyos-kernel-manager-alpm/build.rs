//! Build-time cross-checks for the libalpm FFI (feature `libalpm`).
//!
//! The FFI hardcodes two facts about libalpm. This script verifies them
//! against the ACTUAL system header at build time, so a future libalpm that
//! changes the ABI fails loudly here instead of corrupting package state:
//!
//! 1. `ALPM_SIG_USE_DEFAULT == (1 << 30)`
//! 2. `alpm_pkg_get_installed_db` is declared (the oracle compiles this in
//!    only when `HAVE_ALPM_INSTALLED_DB` is set by its CMake check; we
//!    require it unconditionally and document that in `ffi.rs`).

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_FEATURE_LIBALPM").is_err() {
        return; // FFI not compiled; nothing to verify or link
    }

    // link against the system libalpm (pkg-config is authoritative for the
    // library name and search paths)
    let libs = Command::new("pkg-config")
        .args(["--libs", "libalpm"])
        .output()
        .expect("pkg-config must be installed");
    assert!(libs.status.success(), "pkg-config --libs libalpm failed");
    for flag in String::from_utf8_lossy(&libs.stdout).split_whitespace() {
        if let Some(lib) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib=dylib={lib}");
        } else if let Some(dir) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={dir}");
        }
    }

    let include_dir = pkg_config_include_dir("libalpm");
    let header = include_dir.join("alpm.h");
    let content = std::fs::read_to_string(&header)
        .unwrap_or_else(|e| panic!("cannot read {header:?}: {e} (libalpm development files required for the `libalpm` feature)"));

    check_contains(
        &content,
        "ALPM_SIG_USE_DEFAULT",
        "(1 << 30)",
        "ALPM_SIG_USE_DEFAULT changed; update crates/cachyos-kernel-manager-alpm/src/ffi.rs",
    );
    check_contains(
        &content,
        "alpm_pkg_get_installed_db",
        "alpm_pkg_get_installed_db(alpm_pkg_t *pkg)",
        "alpm_pkg_get_installed_db missing; update crates/cachyos-kernel-manager-alpm/src/ffi.rs",
    );
}

fn pkg_config_include_dir(pkg: &str) -> PathBuf {
    let out = Command::new("pkg-config")
        .args(["--cflags", pkg])
        .output()
        .expect("pkg-config must be installed");
    assert!(out.status.success(), "pkg-config --cflags {pkg} failed");
    let flags = String::from_utf8_lossy(&out.stdout);
    for flag in flags.split_whitespace() {
        if let Some(dir) = flag.strip_prefix("-I") {
            return PathBuf::from(dir);
        }
    }
    PathBuf::from("/usr/include")
}

fn check_contains(content: &str, symbol: &str, needle: &str, message: &str) {
    let found = content
        .lines()
        .any(|l| l.contains(symbol) && l.contains(needle));
    assert!(found, "libalpm header mismatch: {message}");
}
