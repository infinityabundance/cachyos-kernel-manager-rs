//! `cachyos-kernel-manager-mainwindow` — candidate witness for the
//! `ui/main-window-semantics` court. Reads the corpus (kernels, installed
//! provenance, toggle row indices, the sched-ext state file existence, the
//! transaction flag) and renders the candidate's REAL main-window model
//! (`crates/cachyos-kernel-manager-ui/src/main_window.rs`): the tree rows
//! (raw/version/category/checked/immutable/update), the OK-button
//! enablement, the sched-ext button visibility, the change list, the
//! version sort keys, and the space-toggle guard.
//!
//! Usage: cachyos-kernel-manager-mainwindow parse <corpus.json>

use cachyos_kernel_manager_core::discovery::DiscoveredKernel;
use cachyos_kernel_manager_core::selection::SelectionState;
use cachyos_kernel_manager_ui::main_window::{
    execute_enabled, rows, schedext_visible, toggle_allowed, version_sort_key,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    kernels: Vec<DiscoveredKernel>,
    /// name -> (installed_db, version); installed_db null = unknown.
    #[serde(default)]
    installed: BTreeMap<String, (Option<String>, String)>,
    /// Row indices to toggle (in order, before computing the change list).
    #[serde(default)]
    toggles: Vec<usize>,
    #[serde(default)]
    state_file_exists: bool,
    #[serde(default)]
    transaction_running: bool,
}

fn vercmp(a: &str, b: &str) -> std::cmp::Ordering {
    // the court's corpus is vercmp-free (equal versions); use a numeric
    // fallback so the rows are deterministic. vercmp(a, b) > 0 iff a > b
    // (alpm semantics; the exact alpm vercmp is courted by the
    // version-state/epoch courts).
    let num = |s: &str| {
        s.split(['-', '.'])
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };
    num(a).cmp(&num(b))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: cachyos-kernel-manager-mainwindow parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: cachyos-kernel-manager-mainwindow parse <corpus.json>");
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
    let installed = |name: &str| {
        corpus
            .installed
            .get(name)
            .map(|(db, version)| (db.clone(), version.clone()))
    };
    // the view rows: the DEFAULT checked state, then the corpus toggles
    // applied (a repeated index toggles twice, returning to the default)
    let mut view_rows = rows(&corpus.kernels, installed, vercmp);
    for row in &corpus.toggles {
        if let Some(v) = view_rows.get_mut(*row) {
            v.checked = !v.checked;
        }
    }
    let selection = SelectionState {
        rows: view_rows
            .iter()
            .map(|v| {
                let name = v.raw.split('/').nth(1).unwrap_or(&v.raw).to_string();
                cachyos_kernel_manager_core::selection::KernelRow {
                    raw: v.raw.clone(),
                    installed: corpus.installed.contains_key(&name),
                    name,
                    immutable: v.immutable,
                    update_available: v.update_available,
                    checked: v.checked,
                }
            })
            .collect(),
    };
    let sort_keys: Vec<String> = view_rows
        .iter()
        .map(|r| version_sort_key(&r.version_text).to_string())
        .collect();
    let payload = json!({
        "schema": "cachyos-km-main-window-v1",
        "rows": view_rows,
        "execute_enabled": execute_enabled(&selection, corpus.transaction_running),
        "schedext_visible": schedext_visible(corpus.state_file_exists),
        "change_list": selection.change_list(),
        "version_sort_keys": sort_keys,
        "toggle_allowed": toggle_allowed(true, true),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
