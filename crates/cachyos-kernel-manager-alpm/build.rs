//! Build-time ABI court for the libalpm FFI (feature `libalpm`).
//!
//! The FFI hardcodes ABI facts that have already caused two real bugs
//! (list-layout OOM; `installed_db` SIGSEGV). Every one of those facts is
//! now verified against the ACTUAL system headers at build time:
//!
//! 1. `abi/probe.c` is compiled with `-Werror` — its `_Static_assert`s and
//!    function-pointer signature checks fail the build if the header drifts
//!    (layout of `alpm_list_t`, enum sizes, `ALPM_SIG_USE_DEFAULT`, and the
//!    exact return/argument types of every `extern "C"` declaration in
//!    `ffi.rs`).
//! 2. The probe is then RUN; its printed constants are checked against the
//!    semantic invariants the Rust side relies on (`sizeof(alpm_list_t) ==
//!    3*sizeof(void*)`, data/prev/next offsets, `alpm_errno_t`/`alpm_siglevel_t`
//!    are `int`-sized, `ALPM_SIG_USE_DEFAULT == (1 << 30)`).
//!
//! The same probe run is court evidence: `alpm-ffi/abi-surface` compares it
//! byte-for-byte against the Rust-side layout constants.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=abi/probe.c");
    if std::env::var("CARGO_FEATURE_LIBALPM").is_err() {
        return; // FFI not compiled; nothing to verify or link
    }

    // link against the system libalpm (pkg-config is authoritative for the
    // library name and search paths)
    let libs = pkg_config("--libs", "libalpm");
    for flag in libs.split_whitespace() {
        if let Some(lib) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib=dylib={lib}");
        } else if let Some(dir) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={dir}");
        }
    }

    let include_dir = pkg_config_include_dir("libalpm");
    verify_abi(&include_dir);
}

/// Compile `abi/probe.c` against the real headers (-Werror: any static
/// assert or signature mismatch fails the build) and run it, checking the
/// printed constants against the Rust-side invariants.
fn verify_abi(include_dir: &Path) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let probe_src = manifest.join("abi/probe.c");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let probe_bin = out_dir.join("alpm-abi-probe");

    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let compile = Command::new(&cc)
        .arg("-Werror")
        .arg(format!("-I{}", include_dir.display()))
        .arg(&probe_src)
        .arg("-lalpm")
        .arg("-o")
        .arg(&probe_bin)
        .output()
        .expect("cc must be available (base-devel)");
    assert!(
        compile.status.success(),
        "libalpm ABI probe FAILED to compile — the system headers drifted from the FFI assumptions.\n\
         stderr:\n{}\n\
         Update src/ffi.rs (and/or abi/probe.c) to match. Court: alpm-ffi/abi-surface.",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&probe_bin).output().expect("run abi probe");
    assert!(
        run.status.success(),
        "libalpm ABI probe crashed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = String::from_utf8_lossy(&run.stdout);

    // parse the constants and check the Rust-side invariants
    let val = |key: &str| -> u64 {
        out.lines()
            .find_map(|l| {
                l.strip_prefix(&format!("{key}="))
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or_else(|| panic!("ABI probe output missing {key}:\n{out}"))
    };
    let ptr = val("sizeof(void*)");
    assert_eq!(val("sizeof(alpm_list_t)"), 3 * ptr, "alpm_list_t must be data/prev/next (three pointers); update ffi.rs RawList (court alpm-ffi/abi-surface)");
    assert_eq!(
        val("offsetof(alpm_list_t,data)"),
        0,
        "alpm_list_t.data offset; update ffi.rs RawList"
    );
    assert_eq!(
        val("offsetof(alpm_list_t,prev)"),
        ptr,
        "alpm_list_t.prev offset; update ffi.rs RawList"
    );
    assert_eq!(
        val("offsetof(alpm_list_t,next)"),
        2 * ptr,
        "alpm_list_t.next offset; update ffi.rs RawList"
    );
    assert_eq!(
        val("sizeof(alpm_errno_t)"),
        4,
        "alpm_errno_t must be int-sized (c_int); update ffi.rs"
    );
    assert_eq!(
        val("sizeof(alpm_siglevel_t)"),
        4,
        "alpm_siglevel_t must be int-sized (c_int); update ffi.rs"
    );
    assert_eq!(
        val("ALPM_SIG_USE_DEFAULT"),
        1u64 << 30,
        "ALPM_SIG_USE_DEFAULT changed; update ffi.rs"
    );
    println!("cargo:warning=libalpm ABI probe OK (alpm_list_t layout, enums, signatures, ALPM_SIG_USE_DEFAULT)");
}

fn pkg_config(flag: &str, pkg: &str) -> String {
    let out = Command::new("pkg-config")
        .args([flag, pkg])
        .output()
        .expect("pkg-config must be installed");
    assert!(out.status.success(), "pkg-config {flag} {pkg} failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn pkg_config_include_dir(pkg: &str) -> PathBuf {
    let flags = pkg_config("--cflags", pkg);
    for flag in flags.split_whitespace() {
        if let Some(dir) = flag.strip_prefix("-I") {
            return PathBuf::from(dir);
        }
    }
    PathBuf::from("/usr/include")
}
