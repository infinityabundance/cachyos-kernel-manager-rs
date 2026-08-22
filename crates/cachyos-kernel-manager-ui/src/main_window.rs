//! The main window's semantic model — the `ui/main-window-semantics`
//! court's candidate side.
//!
//! Reconstructed from `km-window.cpp` (revision `6b4a373e`):
//! - the tree rows (`init_kernels_tree_widget`, `km-window.cpp:89-106`):
//!   raw, version text (`Kernel::version`, `kernel.cpp:56-79`), category,
//!   checked/immutable flags, the installed-db provenance skip;
//! - the OK button enablement (`build_change_list`, `km-window.cpp:304-325`
//!   + the worker's enable/disable at 125/174);
//! - the sched-ext button visibility (`km-window.cpp:185-188`);
//! - the space-key toggle (`check_uncheck_item`, `km-window.cpp:285-293`);
//! - the version-column sort (`KernelTreeWidgetItem::operator<`,
//!   `km-window.cpp:391-412`: the ∨/∧ markers stripped, alpm vercmp).

use crate::KernelRowView;
use cachyos_kernel_manager_core::discovery::DiscoveredKernel;
use cachyos_kernel_manager_core::kernel::{classify_category, DisplayVersion};
use cachyos_kernel_manager_core::selection::SelectionState;

/// The version text for one row — `Kernel::version` (`kernel.cpp:56-79`):
/// AUR rows short-circuit to `unknown-version`; installed rows compare via
/// vercmp (`∨<local>` downgrade / `∧<sync>` update); uninstalled rows show
/// the sync version.
pub fn version_text(
    kernel: &DiscoveredKernel,
    installed_version: Option<&str>,
    vercmp: impl Fn(&str, &str) -> std::cmp::Ordering,
) -> (String, bool) {
    if kernel.repo == "aur" {
        return ("unknown-version".to_string(), false);
    }
    let display = match installed_version {
        Some(local) => DisplayVersion::compute(Some(local), &kernel.version, vercmp),
        None => DisplayVersion::compute(None, &kernel.version, |_, _| std::cmp::Ordering::Equal),
    };
    (display.text, display.update)
}

/// Build the tree rows exactly like `init_kernels_tree_widget`
/// (`km-window.cpp:89-106`): checked defaults to installed && immutable
/// (installed-db matches the repo or is unknown; an installed-db from a
/// DIFFERENT repo leaves the row mutable and unchecked).
pub fn rows(
    kernels: &[DiscoveredKernel],
    installed: impl Fn(&str) -> Option<(Option<String>, String)>,
    vercmp: impl Fn(&str, &str) -> std::cmp::Ordering,
) -> Vec<KernelRowView> {
    kernels
        .iter()
        .map(|k| {
            let local = installed(&k.name);
            let (version_text, update) = match &local {
                Some((_db, version)) => version_text(k, Some(version), &vercmp),
                None => version_text(k, None, &vercmp),
            };
            let immutable = local.as_ref().is_some_and(|(db, _)| match db {
                None => true,
                Some(db) => db == &k.repo,
            });
            KernelRowView {
                raw: k.raw.clone(),
                version_text,
                category: classify_category(&k.name).display().to_string(),
                checked: local.is_some() && immutable,
                immutable,
                update_available: update,
            }
        })
        .collect()
}

/// The version-column sort key — `KernelTreeWidgetItem::operator<`
/// (`km-window.cpp:391-412`): the ∨/∧ markers are stripped, then alpm
/// vercmp decides.
pub fn version_sort_key(version_text: &str) -> &str {
    version_text
        .strip_prefix('∨')
        .or_else(|| version_text.strip_prefix('∧'))
        .unwrap_or(version_text)
}

/// `check_uncheck_item` (`km-window.cpp:285-293`): the space-key/double-click
/// toggle applies ONLY to leaf items (no children) of the focused tree.
/// The candidate's tree is flat, so every row is a leaf; the guard is
/// modeled for parity (a nested-row shape would not toggle).
pub fn toggle_allowed(row_is_leaf: bool, has_focus: bool) -> bool {
    has_focus && row_is_leaf
}

/// The OK-button enablement: `build_change_list` enables it when the change
/// list becomes non-empty and disables it when empty (`km-window.cpp:307-324`),
/// the worker disables it during a transaction (125) and re-enables it after
/// only when nothing changed (174).
pub fn execute_enabled(selection: &SelectionState, transaction_in_progress: bool) -> bool {
    !transaction_in_progress && !selection.change_list().is_empty()
}

/// The sched-ext button visibility (`km-window.cpp:185-188`).
pub fn schedext_visible(state_file_exists: bool) -> bool {
    state_file_exists
}

/// The full main-window model.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MainWindowModel {
    pub rows: Vec<KernelRowView>,
    pub execute_enabled: bool,
    pub schedext_visible: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_kernel_manager_core::discovery::CompanionNames;

    fn kernel(repo: &str, name: &str, version: &str) -> DiscoveredKernel {
        DiscoveredKernel {
            repo: repo.to_string(),
            name: name.to_string(),
            headers: format!("{name}-headers"),
            version: version.to_string(),
            companions: CompanionNames::default(),
            raw: format!("{repo}/{name}"),
        }
    }

    fn vercmp(a: &str, b: &str) -> std::cmp::Ordering {
        // a tiny version comparison for the tests (numeric pkgrel):
        // vercmp(a, b) > 0 iff a > b (alpm semantics)
        let num = |s: &str| {
            s.split(['-', '.'])
                .filter_map(|p| p.parse::<u32>().ok())
                .collect::<Vec<_>>()
        };
        num(a).cmp(&num(b))
    }

    #[test]
    fn version_text_matches_oracle() {
        let k = kernel("cachyos", "linux-cachyos", "6.14.1-3");
        // installed, equal -> sync version
        let (text, update) = version_text(&k, Some("6.14.1-3"), vercmp);
        assert_eq!(text, "6.14.1-3");
        assert!(!update);
        // installed, local newer -> ∨<local>
        let (text, update) = version_text(&k, Some("6.14.2-1"), vercmp);
        assert_eq!(text, "∨6.14.2-1");
        assert!(!update);
        // installed, local older -> ∧<sync> + update flag
        let (text, update) = version_text(&k, Some("6.13.0-1"), vercmp);
        assert_eq!(text, "∧6.14.1-3");
        assert!(update);
        // not installed -> sync version
        let (text, update) = version_text(&k, None, vercmp);
        assert_eq!(text, "6.14.1-3");
        assert!(!update);
        // aur -> unknown-version, never an update
        let aur = kernel("aur", "linux-cachyos-zen", "unknown-version");
        let (text, update) = version_text(&aur, Some("6.14.1-3"), vercmp);
        assert_eq!(text, "unknown-version");
        assert!(!update);
    }

    #[test]
    fn rows_compute_flags_and_immutability() {
        let kernels = vec![kernel("cachyos", "linux-cachyos", "6.14.1-3")];
        let installed = |name: &str| {
            if name == "linux-cachyos" {
                Some((Some("cachyos".to_string()), "6.14.1-3".to_string()))
            } else {
                None
            }
        };
        let view = rows(&kernels, installed, vercmp);
        assert_eq!(view.len(), 1);
        assert!(view[0].checked);
        assert!(view[0].immutable);
        assert_eq!(view[0].version_text, "6.14.1-3");

        // installed from a DIFFERENT repo -> mutable + unchecked
        let installed = |name: &str| {
            if name == "linux-cachyos" {
                Some((Some("other-repo".to_string()), "6.14.1-3".to_string()))
            } else {
                None
            }
        };
        let view = rows(&kernels, installed, vercmp);
        assert!(!view[0].checked);
        assert!(!view[0].immutable);
    }

    #[test]
    fn version_sort_strips_markers() {
        assert_eq!(version_sort_key("∨6.14.2-1"), "6.14.2-1");
        assert_eq!(version_sort_key("∧6.14.1-3"), "6.14.1-3");
        assert_eq!(version_sort_key("6.14.1-3"), "6.14.1-3");
        assert_eq!(version_sort_key("unknown-version"), "unknown-version");
    }

    #[test]
    fn toggle_guard_requires_leaf_and_focus() {
        assert!(toggle_allowed(true, true));
        assert!(!toggle_allowed(false, true));
        assert!(!toggle_allowed(true, false));
    }

    #[test]
    fn execute_enabled_follows_change_list() {
        let mut sel = SelectionState::default();
        assert!(!execute_enabled(&sel, false));
        sel.rows
            .push(cachyos_kernel_manager_core::selection::KernelRow {
                raw: "cachyos/linux-cachyos".into(),
                name: "linux-cachyos".into(),
                installed: false,
                immutable: false,
                update_available: false,
                checked: true,
            });
        assert!(execute_enabled(&sel, false));
        assert!(!execute_enabled(&sel, true));
    }
}
