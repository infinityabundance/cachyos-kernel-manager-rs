//! `cachyos-kernel-manager-mutate` — candidate mutation witness for the
//! `patch-injection/source-array` and `custom-name/pkgbase-injection` courts.
//!
//! Reproduces the oracle's `on_execute` PKGBUILD mutations
//! (`conf-window.cpp:716-729`) against the FIXTURE PKGBUILD:
//!   1. probes the source array (the oracle's `.testscript` with the default
//!      options env in scope) to obtain the original entries,
//!   2. splices a new `source=(...)` block before `prepare()` — original
//!      entries minus `*.patch`, then the patches list (the fixture's own
//!      `.patch` entries + the user-added URLs) — via
//!      [`insert_patch_source_array`],
//!   3. inserts `\n\npkgbase="<custom_name>"` before `_major=` via
//!      [`insert_custom_pkgbase`], and prints the mutated PKGBUILD text.
//!
//! Usage: cachyos-kernel-manager-mutate <pkgbuild> <custom-name> [patch-url ...]

use cachyos_kernel_manager_build::{
    insert_custom_pkgbase, insert_patch_source_array, options_env_string,
    parse_source_array_probe_output, source_array_probe_script,
};
use cachyos_kernel_manager_core::options::BuildOptions;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(pkgbuild) = args.first() else {
        eprintln!("usage: cachyos-kernel-manager-mutate <pkgbuild> <custom-name> [patch-url ...]");
        return ExitCode::from(2);
    };
    let custom_name = args.get(1).map(String::as_str).unwrap_or("");
    let added_patches: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();

    let text = match std::fs::read_to_string(pkgbuild) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // 1. the source-array probe: the oracle's .testscript text with the
    //    default (untouched) window options env in scope, run via bash
    let env = options_env_string(&BuildOptions::default());
    let script = source_array_probe_script(&env);
    let dir = std::env::temp_dir().join(format!("mutate-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join(".testscript");
    std::fs::write(&script_path, &script).unwrap();
    let out = Command::new("bash")
        .arg(&script_path)
        .arg(pkgbuild)
        .output()
        .expect("run probe");
    let _ = std::fs::remove_dir_all(&dir);
    let mut probe_out = String::from_utf8_lossy(&out.stdout).into_owned();
    // popen parity (utils.cpp:99-120): exactly ONE trailing newline is
    // stripped before the oracle splits on spaces
    if probe_out.ends_with('\n') {
        probe_out.pop();
    }
    let orig_source = parse_source_array_probe_output(&probe_out);

    // 2+3. the patches list = the probe's own .patch entries (the patches
    //    tab is seeded from them at window open) + the user-added URLs
    let mut patches: Vec<String> = orig_source
        .iter()
        .filter(|e| e.ends_with(".patch"))
        .cloned()
        .collect();
    for url in &added_patches {
        patches.push((*url).to_string());
    }

    let spliced = insert_patch_source_array(&text, &orig_source, &patches);
    let mutated = insert_custom_pkgbase(&spliced, custom_name);
    print!("{mutated}");
    ExitCode::SUCCESS
}
