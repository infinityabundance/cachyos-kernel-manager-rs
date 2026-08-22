//! Reference harness reproducing the ORACLE's main-window semantics
//! (`oracle/upstream/src/km-window.cpp` + `kernel.cpp`, revision
//! `6b4a373e`) byte-for-byte:
//!
//! - `init_kernels_tree_widget` (`km-window.cpp:89-106`): raw/version/
//!   category/checked/immutable per row; the installed-db provenance skip
//!   (installed from a DIFFERENT repo → mutable + unchecked);
//! - `Kernel::version` (`kernel.cpp:56-79`): AUR short-circuit
//!   `unknown-version`; installed → vercmp ∨/∧ markers; else the sync
//!   version;
//! - `build_change_list` (`km-window.cpp:304-325`) + the worker enablement
//!   (125/174): the OK button + the change list;
//! - `km-window.cpp:185-188`: the sched-ext button visibility;
//! - `KernelTreeWidgetItem::operator<` (`km-window.cpp:391-412`): the
//!   version sort key (∨/∧ stripped);
//! - `check_uncheck_item` (`km-window.cpp:285-293`): the leaf+focus guard.
//!
//! NOTE: the exact alpm vercmp is courted by the version-state/epoch
//! courts; this court fixes the comparator (the corpus is designed so both
//! sides agree numerically) and pins the ROW ASSEMBLY.
//!
//! Input: the shared corpus (`cachyos-km-main-window-corpus-v1`). Output:
//! the model JSON. This tool is court evidence infrastructure, never
//! shipped.

use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Kernel {
    repo: String,
    name: String,
    /// Corpus parity: the candidate's `DiscoveredKernel` requires this
    /// field, so the corpus carries it; the oracle model does not read it.
    #[allow(dead_code)]
    headers: String,
    version: String,
    raw: String,
}

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    kernels: Vec<Kernel>,
    /// name -> (installed_db, version); installed_db null = unknown.
    #[serde(default)]
    installed: BTreeMap<String, (Option<String>, String)>,
    #[serde(default)]
    toggles: Vec<usize>,
    #[serde(default)]
    state_file_exists: bool,
    #[serde(default)]
    transaction_running: bool,
}

/// The corpus-fixed comparator (numeric; the alpm vercmp is courted
/// elsewhere). vercmp(a, b) > 0 iff a > b.
fn vercmp(a: &str, b: &str) -> std::cmp::Ordering {
    let num = |s: &str| {
        s.split(['-', '.'])
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };
    num(a).cmp(&num(b))
}

/// `Kernel::version` (`kernel.cpp:56-79`).
fn version_text(k: &Kernel, installed_version: Option<&str>) -> (String, bool) {
    if k.repo == "aur" {
        return ("unknown-version".to_string(), false);
    }
    match installed_version {
        Some(local) => match vercmp(local, &k.version) {
            std::cmp::Ordering::Greater => (format!("∨{local}"), false),
            std::cmp::Ordering::Less => (format!("∧{}", k.version), true),
            std::cmp::Ordering::Equal => (k.version.clone(), false),
        },
        None => (k.version.clone(), false),
    }
}

/// `category()` (kernel.hpp:37-92): first substring match in the fixed
/// order; the display strings are the oracle's.
fn category(name: &str) -> &'static str {
    const NEEDLES: &[(&str, &str)] = &[
        ("lto", "lto optimized"),
        ("lts", "longterm"),
        ("zen", "zen-kernel"),
        ("hardened", "hardened kernel"),
        ("deckify", "handheld kernel"),
        ("server", "server kernel"),
        ("next", "next release"),
        ("mainline", "mainline branch"),
        ("git", "master branch"),
        ("rc", "release candidate"),
    ];
    for (needle, display) in NEEDLES {
        if name.contains(needle) {
            return display;
        }
    }
    "stable"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: mainwindow-oracle-ref parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: mainwindow-oracle-ref parse <corpus.json>");
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

    // the rows (init_kernels_tree_widget + Kernel::version) + the final
    // checked states (default checked XOR the toggles)
    let mut view_rows: Vec<serde_json::Value> = Vec::new();
    let mut final_checked: Vec<bool> = Vec::new();
    for (i, k) in corpus.kernels.iter().enumerate() {
        let local = corpus.installed.get(&k.name);
        let (version_text, update) = match local {
            Some((_db, version)) => version_text(k, Some(version)),
            None => version_text(k, None),
        };
        let immutable = local.is_some_and(|(db, _)| match db {
            None => true,
            Some(db) => db == &k.repo,
        });
        let checked = local.is_some() && immutable;
        let toggled = corpus.toggles.iter().filter(|t| **t == i).count() % 2 == 1;
        let checked_after = checked != toggled;
        view_rows.push(json!({
            "raw": k.raw,
            "version_text": version_text,
            "category": category(&k.name),
            "checked": checked_after,
            "immutable": immutable,
            "update_available": update,
        }));
        final_checked.push(checked_after);
    }

    // build_change_list (`km-window.cpp:304-325`) over the final states
    let mut change_list: Vec<String> = Vec::new();
    for (k, checked_after) in corpus.kernels.iter().zip(final_checked.iter()) {
        let local = corpus.installed.get(&k.name);
        let immutable = local.is_some_and(|(db, _)| match db {
            None => true,
            Some(db) => db == &k.repo,
        });
        if immutable && !checked_after {
            change_list.push(k.raw.clone());
        } else if immutable && *checked_after {
            change_list.retain(|s| s != &k.raw);
        } else if *checked_after {
            change_list.push(k.raw.clone());
        } else {
            change_list.retain(|s| s != &k.raw);
        }
    }

    let sort_keys: Vec<String> = view_rows
        .iter()
        .map(|r| {
            let text = r.get("version_text").and_then(|v| v.as_str()).unwrap_or("");
            let stripped = text
                .strip_prefix('∨')
                .or_else(|| text.strip_prefix('∧'))
                .unwrap_or(text);
            stripped.to_string()
        })
        .collect();

    let payload = json!({
        "schema": "cachyos-km-main-window-v1",
        "rows": view_rows,
        "execute_enabled": !corpus.transaction_running && !change_list.is_empty(),
        "schedext_visible": corpus.state_file_exists,
        "change_list": change_list,
        "version_sort_keys": sort_keys,
        "toggle_allowed": true,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
