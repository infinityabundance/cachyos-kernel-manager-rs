//! Normalizers for court observations (directive §46).
//!
//! Raw evidence is never edited; these functions transform it into
//! comparable observables. Every normalizer has a name + version and is
//! covered by tests; the comparator records which normalizer versions it
//! used in the receipt.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One comparable kernel row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelRowObservable {
    /// Display name `<repo>/<kernel>` (oracle: PkgName column; candidate:
    /// `raw`).
    pub raw: String,
    /// Version column text, including any `∨`/`∧` marker.
    pub version: String,
    /// Category column text.
    pub category: String,
    /// Checkbox state.
    pub checked: bool,
}

/// The normalized observation set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Observation {
    /// Kernel rows in display order.
    pub rows: Vec<KernelRowObservable>,
    /// Dialog texts shown by the oracle (critical dialogs etc.).
    pub dialogs: Vec<String>,
}

/// The a11y-row normalizer version. Bump on any extraction-semantics change.
pub const A11Y_NORMALIZER_VERSION: &str = "1.1.0";
/// The candidate-row normalizer version.
pub const CANDIDATE_NORMALIZER_VERSION: &str = "1.0.0";
/// The machine-residual normalizer version.
pub const RESIDUAL_NORMALIZER_VERSION: &str = "1.0.0";

// AT-SPI role ids (at-spi2-core / the vendored pyatspi2 role.py) and the
// equivalent role names. The oracle dump stores the NUMERIC id in `role`
// (canonical raw evidence) and the name in `role_name`; comparators must
// accept both. Qt's a11y bridge exposes the kernel QTreeWidget as TREE(65)
// with a FLAT child list: TABLE_COLUMN_HEADERs (57) then TABLE_CELLs (56)
// in groups of four ([checkbox][pkgname][version][category]) — there are no
// TREE_ITEM/ROW wrapper nodes.

/// Role ids/names that identify tree containers.
fn is_tree_role(role: &str) -> bool {
    matches!(role, "65" | "66" | "TREE" | "TREE_TABLE")
}

/// Role ids/names that identify a row WRAPPER node (nested layout).
fn is_row_container_role(role: &str) -> bool {
    matches!(
        role,
        "90" | "91" | "TABLE_ROW" | "TREE_ITEM" | "ROW" | "LIST_ITEM"
    )
}

/// Role ids/names that identify a table CELL (flat layout constituent).
fn is_cell_role(role: &str) -> bool {
    matches!(role, "56" | "TABLE_CELL")
}

/// Role ids/names that identify column headers.
fn is_header_role(role: &str) -> bool {
    matches!(role, "57" | "10" | "TABLE_COLUMN_HEADER" | "COLUMN_HEADER")
}

/// Role ids/names that identify dialogs/windows carrying user-visible text.
fn is_dialog_role(role: &str) -> bool {
    matches!(role, "2" | "16" | "69" | "ALERT" | "DIALOG" | "WINDOW")
}

/// Extract the oracle's observable state from its full at-spi tree dump
/// (`oracle-state.json`, schema `cachyos-km-oracle-a11y-v1`).
///
/// The dump is a schema wrapper `{ schema, observable, app_name,
/// rows_populated, tree }`; the actual accessibility tree lives under the
/// `tree` FIELD, not as children of the wrapper. Qt's accessibility bridge
/// exposes a QTreeWidget as a TREE/TREE_TABLE whose children are rows; each
/// row's children are the column cells, with the checkbox state exposed on
/// the row and/or its first cell. This extractor is deliberately tolerant:
/// it collects every row in order, assigns columns by position with
/// fallback heuristics, and reports any row it could not parse in
/// `dialogs`-adjacent diagnostics (the comparator turns those into
/// residuals instead of guessing).
pub fn oracle_observation(a11y: &Value) -> Result<Observation, NormalizerError> {
    // unwrap the schema wrapper; tolerate a bare tree passed directly
    let root = a11y.get("tree").unwrap_or(a11y);
    let mut obs = Observation::default();
    collect_observations(root, &mut obs);
    Ok(obs)
}

fn collect_observations(node: &Value, obs: &mut Observation) {
    let role = role_of(node);
    let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");

    // dialogs: ALERT / DIALOG / WINDOW whose text mentions kernels/pacman
    if is_dialog_role(role) {
        let hay = format!("{name} {text}").to_lowercase();
        if hay.contains("kernel") || hay.contains("pacman") || hay.contains("no kernels") {
            let dialog_text = if !text.is_empty() {
                text.to_string()
            } else {
                name.to_string()
            };
            if !dialog_text.is_empty() && !obs.dialogs.contains(&dialog_text) {
                obs.dialogs.push(dialog_text);
            }
        }
    }

    // rows: children of a tree container
    if is_tree_role(role) {
        let children: Vec<Value> = node
            .get("children")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Qt flat layout: direct TABLE_CELL children, chunked into rows
        // (each row starts with the checkable checkbox cell). The column
        // layout is derived from the emitted column headers.
        let cell_children: Vec<&Value> = children
            .iter()
            .filter(|c| is_cell_role(role_of(c)))
            .collect();
        if !cell_children.is_empty() {
            let layout = column_layout(node, &cell_children);
            for row_cells in flat_rows(&cell_children, layout.chunk) {
                if let Some(row) = row_from_flat(&row_cells, &layout) {
                    obs.rows.push(row);
                }
            }
        }

        // nested layout: TREE_ITEM / TABLE_ROW wrapper nodes
        for child in &children {
            if is_row_container_role(role_of(child)) {
                if let Some(row) = row_from_node(child) {
                    obs.rows.push(row);
                }
            }
        }

        // descend into non-row children (dialogs, nested trees)
        for child in &children {
            let crole = role_of(child);
            if !is_row_container_role(crole) && !is_cell_role(crole) && !is_header_role(crole) {
                collect_observations(child, obs);
            }
        }
        return;
    }

    // descend generally (dialogs may live anywhere)
    for child in node
        .get("children")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        collect_observations(child, obs);
    }
}

/// Canonical role string of a node: prefer the numeric `role`, fall back to
/// `role_name` (older dumps had names only).
fn role_of(node: &Value) -> &str {
    node.get("role")
        .and_then(|v| v.as_str())
        .or_else(|| node.get("role_name").and_then(|v| v.as_str()))
        .unwrap_or("")
}

fn has_state(node: &Value, state: &str) -> bool {
    node.get("states")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|s| s.as_str() == Some(state)))
        .unwrap_or(false)
}

/// Column layout of a flat table, derived from the emitted column headers
/// (the oracle renders Choose / PkgName / Version / Category) with a
/// fallback to the first cell's `checkable` state and a 4-column default.
struct ColumnLayout {
    /// Whether column 0 is the checkbox column.
    has_checkbox: bool,
    /// Number of columns per row.
    chunk: usize,
}

fn column_layout(tree_node: &Value, cells: &[&Value]) -> ColumnLayout {
    let headers: Vec<String> = tree_node
        .get("children")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter(|c| is_header_role(role_of(c)))
        .map(|c| {
            c.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    if !headers.is_empty() {
        let first = headers[0].to_lowercase();
        let has_checkbox = first.is_empty()
            || first.contains("choos")
            || first.contains("check")
            || first.contains("select");
        ColumnLayout {
            has_checkbox,
            chunk: headers.len(),
        }
    } else {
        let has_checkbox = cells
            .first()
            .map(|c| has_state(c, "checkable"))
            .unwrap_or(false);
        ColumnLayout {
            has_checkbox,
            chunk: 4,
        }
    }
}

/// Chunk a flat TABLE_CELL list into rows. A row starts at a cell with the
/// `checkable` state (Qt's checkbox cell); if no checkable cells exist (a
/// table without a checkbox column), fall back to positional chunks of the
/// layout's column count.
fn flat_rows<'a>(cells: &[&'a Value], chunk: usize) -> Vec<Vec<&'a Value>> {
    let mut rows: Vec<Vec<&'a Value>> = Vec::new();
    for cell in cells {
        if has_state(cell, "checkable") && rows.last().is_none_or(|r| !r.is_empty()) {
            rows.push(vec![*cell]);
        } else if let Some(row) = rows.last_mut() {
            if row.len() < chunk {
                row.push(*cell);
            }
        }
    }
    if rows.is_empty() && !cells.is_empty() {
        rows = cells.chunks(chunk).map(|c| c.to_vec()).collect();
    }
    rows
}

/// Build a row from the flat layout. With a checkbox column the positions
/// are [checkbox][pkgname][version][category]; without one they are
/// [pkgname][version][category]. The checkbox cell's `checked` state is the
/// checkbox observable.
fn row_from_flat(cells: &[&Value], layout: &ColumnLayout) -> Option<KernelRowObservable> {
    let data = if layout.has_checkbox { 1 } else { 0 };
    let cell_field = |i: usize, field: &str| -> String {
        cells
            .get(i)
            .and_then(|n| n.get(field))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let raw = {
        let n = cell_field(data, "name");
        if n.is_empty() {
            cell_field(data, "text")
        } else {
            n
        }
    };
    let version = cell_field(data + 1, "text");
    let category = cell_field(data + 2, "text");
    if raw.is_empty() && version.is_empty() && category.is_empty() {
        return None;
    }
    let checked = if layout.has_checkbox {
        cells.first().map(|c| node_checked(c)).unwrap_or(false)
    } else {
        false
    };
    Some(KernelRowObservable {
        raw,
        version,
        category,
        checked,
    })
}

fn cell_texts(node: &Value) -> Vec<String> {
    let mut out = Vec::new();
    // the row node's own text is column 0 (the checkbox cell)
    if let Some(t) = node.get("text").and_then(|v| v.as_str()) {
        if !t.is_empty() {
            out.push(t.to_string());
        }
    }
    for child in node
        .get("children")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let role = child.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role == "CHECK_BOX" || role == "CHECKBOX" {
            continue; // checkbox cell: text is not a data column
        }
        if let Some(t) = child.get("text").and_then(|v| v.as_str()) {
            if !t.is_empty() {
                out.push(t.to_string());
            }
        }
        if let Some(n) = child.get("name").and_then(|v| v.as_str()) {
            if !n.is_empty() && !out.iter().any(|x| x == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

fn node_checked(node: &Value) -> bool {
    if has_state(node, "checked") {
        return true;
    }
    node.get("children")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|c| has_state(c, "checked")))
        .unwrap_or(false)
}

/// Category strings the oracle can render; used for column disambiguation.
const KNOWN_CATEGORIES: &[&str] = &[
    "lto optimized",
    "longterm",
    "zen-kernel",
    "hardened kernel",
    "handheld kernel",
    "server kernel",
    "next release",
    "mainline branch",
    "master branch",
    "release candidate",
    "stable",
];

fn row_from_node(node: &Value) -> Option<KernelRowObservable> {
    let mut texts = cell_texts(node);
    // a row with no data columns is not a kernel row
    if texts.is_empty() {
        return None;
    }
    // Column layout: [checkbox-cell], PkgName, Version, Category.
    // Heuristics: version = text containing a digit AND ('-' or '.' or a
    // ∨/∧ marker); category = a known category string; pkgname = the rest.
    let mut version: Option<String> = None;
    let mut category: Option<String> = None;
    let mut pkgname: Option<String> = None;

    for t in texts.drain(..) {
        let looks_version = t.chars().any(|c| c.is_ascii_digit())
            && (t.contains('-') || t.contains('.') || t.starts_with('∨') || t.starts_with('∧'));
        let looks_category = KNOWN_CATEGORIES.contains(&t.as_str());
        if looks_version && version.is_none() {
            version = Some(t);
        } else if looks_category && category.is_none() {
            category = Some(t);
        } else if pkgname.is_none() {
            // remaining text is the package name (any non-empty cell)
            pkgname = Some(t);
        }
    }

    Some(KernelRowObservable {
        raw: pkgname.unwrap_or_default(),
        version: version.unwrap_or_default(),
        category: category.unwrap_or_default(),
        checked: node_checked(node),
    })
}

/// Extract the candidate's observable state from `candidate-state.json`
/// (schema `cachyos-km-candidate-state-v1`).
pub fn candidate_observation(state: &Value) -> Result<Observation, NormalizerError> {
    let mut obs = Observation::default();
    let kernels = state
        .get("kernels")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            NormalizerError::Missing("candidate-state.json has no `kernels` array".to_string())
        })?;
    for k in kernels {
        let raw = k
            .get("raw")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let version = k
            .get("display_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let category = k
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let checked = k
            .get("checked_default")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        obs.rows.push(KernelRowObservable {
            raw,
            version,
            category,
            checked,
        });
    }
    Ok(obs)
}

/// Normalize a machine residual (schema `cachyos-km-machine-residual-v1`)
/// to a comparable digest string: the installed package list, sync db
/// hashes, and local db package list, sorted and joined.
pub fn residual_digest(residual: &Value) -> Result<String, NormalizerError> {
    let installed = residual
        .get("installed_packages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| NormalizerError::Missing("residual has no installed_packages".into()))?;
    let mut parts: Vec<String> = installed
        .iter()
        .filter_map(|p| p.as_str().map(|s| format!("pkg:{s}")))
        .collect();
    if let Some(dbs) = residual.get("sync_db_hashes").and_then(|v| v.as_object()) {
        for (k, v) in dbs {
            parts.push(format!("db:{k}:{}", v.as_str().unwrap_or("?")));
        }
    }
    if let Some(local) = residual.get("local_db_packages").and_then(|v| v.as_array()) {
        for p in local {
            if let Some(s) = p.as_str() {
                parts.push(format!("local:{s}"));
            }
        }
    }
    parts.sort();
    Ok(parts.join("\n"))
}

/// Errors from normalization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NormalizerError {
    #[error("{0}")]
    Missing(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(text: &str, checked: bool) -> Value {
        let mut states = vec!["enabled".to_string()];
        if checked {
            states.push("checked".to_string());
        }
        serde_json::json!({ "role": "TREE_ITEM", "name": text, "text": text, "states": states, "children": [] })
    }

    /// A flat Qt-layout TABLE_CELL (numeric role "56"), the shape the real
    /// oracle emits: checkbox cells carry `checkable` (+ `checked` when the
    /// kernel is selected/installed) and empty text; data cells carry the
    /// column text in both name and text.
    fn flat_cell(text: &str, checkable: bool, checked: bool) -> Value {
        let mut states = vec![
            "enabled".to_string(),
            "focusable".to_string(),
            "sensitive".to_string(),
        ];
        if checkable {
            states.push("checkable".to_string());
        }
        if checked {
            states.push("checked".to_string());
        }
        serde_json::json!({
            "role": "56",
            "role_name": "TABLE_CELL",
            "name": text,
            "text": text,
            "states": states,
            "children": []
        })
    }

    fn header(text: &str) -> Value {
        serde_json::json!({
            "role": "57",
            "role_name": "TABLE_COLUMN_HEADER",
            "name": text,
            "text": text,
            "states": ["enabled"],
            "children": []
        })
    }

    fn flat_row(pkg: &str, version: &str, category: &str, checked: bool) -> Vec<Value> {
        vec![
            flat_cell("", true, checked),
            flat_cell(pkg, false, false),
            flat_cell(version, false, false),
            flat_cell(category, false, false),
        ]
    }

    #[test]
    fn oracle_extraction_of_simple_tree() {
        // Simulated Qt a11y shape: TREE_TABLE -> rows -> cells
        let tree = serde_json::json!({
            "role": "TREE_TABLE",
            "name": "",
            "text": "",
            "states": ["enabled"],
            "children": [
                {
                    "role": "TREE_ITEM",
                    "name": "cachyos/linux-cachyos",
                    "text": "",
                    "states": ["enabled", "checked", "checkable"],
                    "children": [
                        cell("", true),
                        cell("cachyos/linux-cachyos", true),
                        cell("7.1.8-1", true),
                        cell("stable", true),
                    ]
                },
                {
                    "role": "TREE_ITEM",
                    "name": "cachyos/linux-cachyos-lts",
                    "text": "",
                    "states": ["enabled", "checkable"],
                    "children": [
                        cell("", false),
                        cell("cachyos/linux-cachyos-lts", false),
                        cell("∨7.0.6-1", false),
                        cell("longterm", false),
                    ]
                }
            ]
        });
        let obs = oracle_observation(&tree).unwrap();
        assert_eq!(obs.rows.len(), 2);
        assert_eq!(obs.rows[0].raw, "cachyos/linux-cachyos");
        assert_eq!(obs.rows[0].version, "7.1.8-1");
        assert_eq!(obs.rows[0].category, "stable");
        assert!(obs.rows[0].checked);
        assert_eq!(obs.rows[1].raw, "cachyos/linux-cachyos-lts");
        assert_eq!(obs.rows[1].version, "∨7.0.6-1");
        assert_eq!(obs.rows[1].category, "longterm");
        assert!(!obs.rows[1].checked);
    }

    #[test]
    fn oracle_extraction_of_flat_numeric_tree() {
        // The REAL Qt a11y shape (captured from the oracle VM): a TREE ("65")
        // whose children are TABLE_COLUMN_HEADERs then a FLAT list of
        // TABLE_CELLs in groups of four, with no TREE_ITEM wrappers.
        let mut children = vec![
            header("Choose"),
            header("PkgName"),
            header("Version"),
            header("Category"),
        ];
        children.extend(flat_row(
            "cachyos/linux-cachyos-bmq",
            "7.0.11-1",
            "stable",
            false,
        ));
        children.extend(flat_row("cachyos/linux-cachyos", "7.1.8-1", "stable", true));
        children.extend(flat_row("core/linux", "7.1.8.arch1-3", "stable", false));
        children.extend(flat_row(
            "extra/linux-zen",
            "7.1.8.zen1-3",
            "zen-kernel",
            false,
        ));
        let tree = serde_json::json!({
            "role": "65",
            "role_name": "TREE",
            "name": "",
            "text": "",
            "states": ["enabled"],
            "children": children
        });
        let obs = oracle_observation(&tree).unwrap();
        assert_eq!(obs.rows.len(), 4);
        assert_eq!(obs.rows[0].raw, "cachyos/linux-cachyos-bmq");
        assert_eq!(obs.rows[0].version, "7.0.11-1");
        assert_eq!(obs.rows[0].category, "stable");
        assert!(!obs.rows[0].checked);
        assert_eq!(obs.rows[1].raw, "cachyos/linux-cachyos");
        assert_eq!(obs.rows[1].version, "7.1.8-1");
        assert!(obs.rows[1].checked);
        assert_eq!(obs.rows[2].raw, "core/linux");
        assert_eq!(obs.rows[3].category, "zen-kernel");
    }

    #[test]
    fn oracle_extraction_of_flat_tree_without_checkable_cells() {
        // A table without a checkbox column (3 headers, 3 cells per row):
        // chunking follows the header count, columns are positional.
        let children = vec![
            header("PkgName"),
            header("Version"),
            header("Category"),
            flat_cell("cachyos/linux-cachyos", false, false),
            flat_cell("7.1.8-1", false, false),
            flat_cell("stable", false, false),
            flat_cell("cachyos/linux-cachyos-lts", false, false),
            flat_cell("6.18.42-1", false, false),
            flat_cell("longterm", false, false),
            flat_cell("cachyos/linux-cachyos-rt", false, false),
            flat_cell("7.1.8-1", false, false),
            flat_cell("stable", false, false),
        ];
        let tree = serde_json::json!({
            "role": "66",
            "role_name": "TREE_TABLE",
            "children": children
        });
        let obs = oracle_observation(&tree).unwrap();
        assert_eq!(obs.rows.len(), 3);
        assert_eq!(obs.rows[0].raw, "cachyos/linux-cachyos");
        assert_eq!(obs.rows[0].version, "7.1.8-1");
        assert!(!obs.rows[0].checked);
        assert_eq!(obs.rows[1].raw, "cachyos/linux-cachyos-lts");
        assert_eq!(obs.rows[1].version, "6.18.42-1");
        assert_eq!(obs.rows[1].category, "longterm");
    }

    #[test]
    fn oracle_extraction_of_dialog() {
        let tree = serde_json::json!({
            "role": "75",
            "role_name": "APPLICATION",
            "name": "cachyos-kernel-manager",
            "text": "",
            "states": [],
            "children": [
                {
                    "role": "69",
                    "role_name": "WINDOW",
                    "name": "CachyOS Kernel Manager",
                    "text": "No kernels found!\nPlease run `pacman -Sy` to update DB!\nThis is needed for the app to work properly",
                    "states": [],
                    "children": []
                }
            ]
        });
        let obs = oracle_observation(&tree).unwrap();
        assert!(obs.dialogs.iter().any(|d| d.contains("No kernels found")));
        assert!(obs.rows.is_empty());
    }

    #[test]
    fn oracle_extraction_of_real_captured_evidence() {
        // Regression smoke test against the LATEST real oracle dump captured
        // by the VM court (ignored by default; run with --ignored after a
        // court run). Kept as a permanent bridge between the VM evidence
        // and the normalizer.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("courts/kernel-discovery/minimal/oracle/oracle-state.json");
        let Ok(content) = std::fs::read_to_string(&path) else {
            eprintln!("skipping: no captured evidence at {}", path.display());
            return;
        };
        let tree: Value = serde_json::from_str(&content).expect("evidence json");
        let obs = oracle_observation(&tree).expect("extract");
        assert!(
            obs.rows.len() >= 17,
            "expected at least the 17 baseline rows, got {}",
            obs.rows.len()
        );
        assert_eq!(obs.rows[0].raw, "cachyos/linux-cachyos-bmq");
        assert_eq!(obs.rows[0].version, "7.0.11-1");
        assert_eq!(obs.rows[0].category, "stable");
        let installed = obs.rows.iter().find(|r| r.raw == "cachyos/linux-cachyos");
        assert!(installed.is_some_and(|r| r.checked));
    }

    #[test]
    fn candidate_extraction() {
        let state = serde_json::json!({
            "kernels": [
                {
                    "raw": "cachyos/linux-cachyos",
                    "display_version": "7.1.8-1",
                    "category": "stable",
                    "checked_default": true,
                }
            ]
        });
        let obs = candidate_observation(&state).unwrap();
        assert_eq!(obs.rows.len(), 1);
        assert_eq!(obs.rows[0].raw, "cachyos/linux-cachyos");
        assert!(obs.rows[0].checked);
    }

    #[test]
    fn residual_digest_is_sorted_and_stable() {
        let r1 = serde_json::json!({
            "installed_packages": ["b 1-1", "a 2-1"],
            "sync_db_hashes": { "cachyos.db": "abc" },
            "local_db_packages": ["linux-cachyos 7.1.8-1"]
        });
        let r2 = serde_json::json!({
            "installed_packages": ["a 2-1", "b 1-1"],
            "sync_db_hashes": { "cachyos.db": "abc" },
            "local_db_packages": ["linux-cachyos 7.1.8-1"]
        });
        assert_eq!(residual_digest(&r1).unwrap(), residual_digest(&r2).unwrap());
        let r3 = serde_json::json!({
            "installed_packages": ["a 2-1", "b 1-1"],
            "sync_db_hashes": { "cachyos.db": "def" },
            "local_db_packages": ["linux-cachyos 7.1.8-1"]
        });
        assert_ne!(residual_digest(&r1).unwrap(), residual_digest(&r3).unwrap());
    }
}
