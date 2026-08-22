//! `cachyos-kernel-manager-artifact-glob` — candidate witness for the
//! `artifact-glob/package-functions` court.
//!
//! Runs the SAME probe scripts as the oracle (bash is the contract; the
//! script texts come from the frozen source and are asserted by the build
//! crate's tests), then applies the candidate's parse/glob models
//! (`parse_pkgfuncs_probe_output` + `artifact_globs`), emitting the
//! identical JSON as `tools/artifact-oracle-ref`.
//!
//! Usage: cachyos-kernel-manager-artifact-glob probe <pkgbuild> <pkgext|probe>

use cachyos_kernel_manager_build::{
    artifact_globs, parse_pkgfuncs_probe_output, pkgext_probe_script, pkgfuncs_probe_script,
};
use serde_json::json;
use std::process::{Command, ExitCode};

fn run_script(script: &str, arg: Option<&str>) -> String {
    let dir = std::env::temp_dir().join(format!("artifact-candidate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join(".testscript");
    std::fs::write(&script_path, script).unwrap();
    let mut cmd = Command::new("bash");
    cmd.arg(&script_path);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    let out = cmd.output().expect("run probe");
    let _ = std::fs::remove_dir_all(&dir);
    let mut result = String::from_utf8_lossy(&out.stdout).into_owned();
    if result.ends_with('\n') {
        result.pop();
    }
    result
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (pkgbuild, pkgext_input) = match args.as_slice() {
        [cmd, pkgbuild, pkgext] if cmd == "probe" => (pkgbuild.clone(), pkgext.clone()),
        _ => {
            eprintln!(
                "usage: cachyos-kernel-manager-artifact-glob probe <pkgbuild> <pkgext|probe>"
            );
            return ExitCode::from(2);
        }
    };
    let pkgext_val = match pkgext_input.as_str() {
        "probe" => {
            let v = run_script(&pkgext_probe_script(), None);
            if v.is_empty() {
                eprintln!("failed to get PKGEXT from /etc/makepkg.conf");
                ".pkg.tar.zst".to_string()
            } else {
                v
            }
        }
        "probe-empty" => {
            eprintln!("failed to get PKGEXT from /etc/makepkg.conf");
            ".pkg.tar.zst".to_string()
        }
        literal => literal.to_string(),
    };

    let probe_output = run_script(&pkgfuncs_probe_script(), Some(&pkgbuild));
    let (suffixes, pkgver) = parse_pkgfuncs_probe_output(&probe_output);
    let broken = pkgver.is_none();
    let (pkgver_str, globs) = match pkgver {
        Some((ver, rel)) => {
            let pkgver_str = format!("{ver}-{rel}");
            let globs = artifact_globs(&suffixes, &ver, &rel, &pkgext_val);
            (Some(pkgver_str), globs)
        }
        None => (None, Vec::new()),
    };
    let payload = json!({
        "pkgext": pkgext_val,
        "probe_output": probe_output,
        "suffixes": suffixes,
        "pkgver_str": pkgver_str,
        "globs": globs,
        "error": if broken { json!("broken pkgbuild; pkgver must be present") } else { serde_json::Value::Null },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
