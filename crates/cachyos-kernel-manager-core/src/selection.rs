//! Kernel selection and change-list semantics.
//!
//! Reconstructed from `build_change_list` / `init_kernels_tree_widget`
//! (`oracle/upstream/src/km-window.cpp:89-106,304-325`) and
//! `install_packages` / `remove_packages` (`km-window.cpp:48-71`).
//! Protected by courts: `transaction-plan/*`, `ui.main-window`.

/// Tree-column textual markers stored in the hidden columns
/// (`km-window.cpp:96,102` — `QStringLiteral("true")`).
pub const IMMUTABLE: &str = "true";

/// The `Displayed` hidden-column marker (always `true`; the column is
/// hidden and never read back by the oracle).
pub const DISPLAYED: &str = "true";

/// A row in the kernels tree, with the oracle's derived flags.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KernelRow {
    /// Display name `<repo>/<kernel>`.
    pub raw: String,
    /// Package name (no repo).
    pub name: String,
    /// Installed in the local database.
    pub installed: bool,
    /// `Immutable` column marker: installed **and** installed-db matches the
    /// current sync repo (or installed-db is unknown/empty). Rows installed
    /// from a *different* repo stay unchecked and mutable
    /// (`km-window.cpp:97-104`).
    pub immutable: bool,
    /// `m_update` flag: sync version newer than installed.
    pub update_available: bool,
    /// Current checkbox state.
    pub checked: bool,
}

/// The selection state of the tree.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionState {
    /// Rows in tree order (sync-db discovery order).
    pub rows: Vec<KernelRow>,
}

impl SelectionState {
    /// Build the shared change list exactly like `build_change_list`
    /// (`km-window.cpp:304-325`):
    ///
    /// - immutable && unchecked → add (removal candidate)
    /// - immutable && checked   → remove
    /// - mutable   && checked   → add (install candidate)
    /// - mutable   && unchecked → remove
    ///
    /// The oracle keeps one list; install and remove phases both iterate it
    /// and apply their own installed/update filters.
    pub fn change_list(&self) -> Vec<String> {
        let mut list: Vec<String> = Vec::new();
        for row in &self.rows {
            let item_text = row.raw.clone();
            if row.immutable && !row.checked {
                list.push(item_text);
            } else if row.immutable && row.checked {
                list.retain(|s| s != &item_text);
            } else if row.checked {
                list.push(item_text);
            } else {
                list.retain(|s| s != &item_text);
            }
        }
        list
    }

    /// The install phase filter (`install_packages`, `km-window.cpp:48-58`):
    /// a change-list entry is installed only if it is *not installed* or an
    /// *update is available*.
    pub fn install_set(&self) -> Vec<String> {
        self.change_list()
            .into_iter()
            .filter(|raw| {
                self.rows
                    .iter()
                    .find(|r| &r.raw == raw)
                    .is_some_and(|r| !r.installed || r.update_available)
            })
            .collect()
    }

    /// The removal phase filter (`remove_packages`, `km-window.cpp:60-71`):
    /// a change-list entry is removed only if it *is installed*.
    pub fn removal_set(&self) -> Vec<String> {
        self.change_list()
            .into_iter()
            .filter(|raw| self.rows.iter().any(|r| &r.raw == raw && r.installed))
            .collect()
    }
}

/// Space-key / double-click toggle (`check_uncheck_item`, `km-window.cpp:
/// 285-293`): flips the current leaf item's checkbox. In the oracle the
/// current item is the tree's current item and the toggle only applies to
/// leaf items; the model takes the row index.
pub fn toggle_row(state: &mut SelectionState, row_index: usize) {
    if let Some(row) = state.rows.get_mut(row_index) {
        row.checked = !row.checked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(raw: &str, installed: bool, immutable: bool, update: bool, checked: bool) -> KernelRow {
        KernelRow {
            raw: raw.into(),
            name: raw.split('/').nth(1).unwrap_or(raw).into(),
            installed,
            immutable,
            update_available: update,
            checked,
        }
    }

    #[test]
    fn change_list_semantics() {
        // installed, immutable, checked (default) -> not in list
        let s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", true, true, false, true)],
        };
        assert!(s.change_list().is_empty());

        // installed kernel unchecked -> removal
        let s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", true, true, false, false)],
        };
        assert_eq!(s.change_list(), vec!["cachyos/linux-cachyos"]);
        assert!(s.install_set().is_empty());
        assert_eq!(s.removal_set(), vec!["cachyos/linux-cachyos"]);

        // available kernel checked -> install
        let s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", false, false, false, true)],
        };
        assert_eq!(s.change_list(), vec!["cachyos/linux-cachyos"]);
        assert_eq!(s.install_set(), vec!["cachyos/linux-cachyos"]);
        assert!(s.removal_set().is_empty());

        // installed with update available, checked -> reinstall (upgrade)
        let s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", true, true, true, true)],
        };
        assert!(s.change_list().is_empty()); // checked+immutable -> not in list
                                             // to reinstall an updated kernel the user unchecks it, which would
                                             // REMOVE it; oracle behavior: install path only triggers for
                                             // !installed || update — but change_list only contains unchecked
                                             // immutable rows. So update reinstall actually happens through the
                                             // removal path in the oracle (uncheck -> remove). Preserved as-is.

        // toggle twice returns original
        let mut s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", false, false, false, false)],
        };
        toggle_row(&mut s, 0);
        assert!(s.rows[0].checked);
        toggle_row(&mut s, 0);
        assert!(!s.rows[0].checked);
    }

    #[test]
    fn installed_from_other_repo_row_is_mutable_and_unchecked() {
        // km-window.cpp:98-101: installed from a different repo -> row stays
        // in the tree, unchecked, no immutable marker.
        let s = SelectionState {
            rows: vec![row("cachyos/linux-cachyos", true, false, false, false)],
        };
        // it behaves like an install candidate; install_set filters it out
        // because it IS installed and has no update
        assert_eq!(s.install_set(), Vec::<String>::new());
        // but checking it and executing leaves it alone; unchecking it would
        // add to change_list -> removal
        let mut s2 = s.clone();
        s2.rows[0].checked = true;
        assert_eq!(s2.change_list(), vec!["cachyos/linux-cachyos"]);
        assert!(s2.install_set().is_empty()); // installed, no update -> not installed
        assert_eq!(s2.removal_set(), vec!["cachyos/linux-cachyos"]);
    }
}
