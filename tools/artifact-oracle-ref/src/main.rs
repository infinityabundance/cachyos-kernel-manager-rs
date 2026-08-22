//! Reference harness reproducing the ORACLE's artifact-glob pipeline
//! (`conf-window.cpp:218-298`, revision `6b4a373e`):
//!
//! 1. `get_pkgext_value_from_makepkgconf` (218-236): the probe script
//!    `#!/usr/bin/bash\nsource "/etc/makepkg.conf"\necho "${PKGEXT}"` is
//!    written and executed; an EMPTY result falls back to `.pkg.tar.zst`.
//!    For reproducible non-VM evidence the corpus supplies the PKGEXT the
//!    probe would yield (`pkgext`: a literal, or `"probe"` to run the real
//!    script against the host's /etc/makepkg.conf — both sides see the same
//!    file, so the comparison stays about the algorithm).
//! 2. `get_package_names_glob_from_pkgbuild` (274-298): the probe script
//!    `#!/usr/bin/bash\nsource "$1"\ndeclare -F;echo "pkgver: $pkgver-$pkgrel"`
//!    is written and executed against the PKGBUILD; the `pkgver: ` line is
//!    located (missing -> stderr "broken pkgbuild; pkgver must be present"
//!    + empty globs); its suffix is the `pkgver_str`.
//! 3. `prepare_func_names` (238-272): each line strips a `declare -f `
//!    prefix, keeps only `package_`-prefixed functions, strips that prefix,
//!    and renders `{suffix}-{pkgver_str}-*{pkgext}`.
//!
//! Usage: artifact-oracle-ref probe <pkgbuild> <pkgext>
//!   pkgext: a literal PKGEXT value, or "probe" for the real script.
//! This tool is court evidence infrastructure, never shipped.

use serde_json::json;
use std::process::{Command, ExitCode};

const PKGEXT_SCRIPT: &str = "#!/usr/bin/bash\nsource \"/etc/makepkg.conf\"\necho \"${PKGEXT}\"";
const PKGFUNCS_SCRIPT: &str =
    "#!/usr/bin/bash\nsource \"$1\"\ndeclare -F;echo \"pkgver: $pkgver-$pkgrel\"";

/// Write the script to a private temp file and run it with bash
/// (popen semantics: stdout captured, exactly one trailing newline
/// stripped).
fn run_script(script: &str, arg: Option<&str>) -> String {
    let dir = std::env::temp_dir().join(format!("artifact-oracle-ref-{}", std::process::id()));
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

/// get_pkgext_value_from_makepkgconf (`conf-window.cpp:218-236`).
/// `pkgext` corpus field: "probe" runs the real script (fallback when the
/// host's /etc/makepkg.conf yields empty), "probe-empty" simulates an
/// empty probe result (exercising the .pkg.tar.zst fallback), any other
/// string is the literal PKGEXT the probe would yield.
fn pkgext_value(pkgext_input: &str) -> String {
    match pkgext_input {
        "probe" => {
            let v = run_script(PKGEXT_SCRIPT, None);
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
    }
}

/// get_package_names_glob_from_pkgbuild + prepare_func_names
/// (`conf-window.cpp:274-298,238-272`). Returns
/// (probe_output, suffixes, pkgver_str, globs, broken).
fn pkg_globs(pkgbuild: &str, pkgext_val: &str) -> (String, Vec<String>, String, Vec<String>, bool) {
    let probe_out = run_script(PKGFUNCS_SCRIPT, Some(pkgbuild));
    let parse_lines: Vec<&str> = probe_out.split('\n').filter(|s| !s.is_empty()).collect();

    let pkgver_line = parse_lines.iter().find(|line| line.starts_with("pkgver: "));
    let Some(pkgver_line) = pkgver_line else {
        eprintln!("broken pkgbuild; pkgver must be present");
        return (probe_out, Vec::new(), String::new(), Vec::new(), true);
    };
    let pkgver_str = pkgver_line["pkgver: ".len()..].to_string();

    let mut suffixes = Vec::new();
    let mut globs = Vec::new();
    for line in &parse_lines {
        let mut line = *line;
        if let Some(rest) = line.strip_prefix("declare -f ") {
            line = rest;
        }
        if let Some(suffix) = line.strip_prefix("package_") {
            suffixes.push(suffix.to_string());
            globs.push(format!("{suffix}-{pkgver_str}-*{pkgext_val}"));
        }
    }
    (probe_out, suffixes, pkgver_str, globs, false)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (pkgbuild, pkgext_input) = match args.as_slice() {
        [cmd, pkgbuild, pkgext] if cmd == "probe" => (pkgbuild.clone(), pkgext.clone()),
        _ => {
            eprintln!("usage: artifact-oracle-ref probe <pkgbuild> <pkgext|probe>");
            return ExitCode::from(2);
        }
    };
    let pkgext_val = pkgext_value(&pkgext_input);
    let (probe_output, suffixes, pkgver_str, globs, broken) = pkg_globs(&pkgbuild, &pkgext_val);
    let payload = json!({
        "pkgext": pkgext_val,
        "probe_output": probe_output,
        "suffixes": suffixes,
        "pkgver_str": if broken { serde_json::Value::Null } else { json!(pkgver_str) },
        "globs": globs,
        "error": if broken { json!("broken pkgbuild; pkgver must be present") } else { serde_json::Value::Null },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
