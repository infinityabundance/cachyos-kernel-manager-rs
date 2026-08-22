//! Application state machine (Phase 8 — orthogonal state).
//!
//! The oracle's implicit state (constructor → worker thread → refresh) is
//! made explicit here as pure transitions `State + Event → (State, Effects)`
//! so it can be replayed, tested, and shrunk (directive §8).
//!
//! Phase 8 restructure (the Phase-8-start note): the single coarse
//! `AppPhase` enum became too coarse for the real application — reality
//! permits PARTIALLY INDEPENDENT state, e.g. `catalog = Loaded`,
//! `selection = Dirty`, `build = Running`, `scx = Visible`,
//! `dialog = Progress(...)` at the same time. The authoritative state is
//! therefore orthogonal: [`AppState`] carries one component per concern
//! ([`LifecycleState`], [`CatalogState`], [`SelectionState`],
//! [`TransactionState`], [`ConfigurationState`], [`BuildState`],
//! [`ScxState`], [`DialogsState`]). [`AppPhase`] remains as the DERIVED
//! coarse projection (the docs' state table) — `AppState::phase()` computes
//! it from the components; it is never stored.

use crate::options::BuildOptions;
use crate::selection::SelectionState;

/// The coarse phase projection (docs/ARCHITECTURE.md §State machine).
/// Derived from the orthogonal components via [`AppState::phase`]; NEVER
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // phase semantics documented in docs/ARCHITECTURE.md §State machine
pub enum AppPhase {
    Startup,
    KernelDiscovery,
    Ready,
    SelectionChanged,
    TransactionPlanning,
    Authenticating,
    TransactionRunning,
    TransactionComplete,
    TransactionFailed,
    RefreshingPackageState,
    ConfigurationPreparation,
    ConfigurationEditing,
    BuildRunning,
    BuildFailed,
    BuildCompleted,
    ArtifactInstallation,
    ScxConfiguration,
    Shutdown,
}

/// The application lifecycle (constructor → ready → shutdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum LifecycleState {
    /// `main()` startup (lock, QApplication init).
    Startup,
    /// Blocking in the MainWindow ctor (oracle); async in the candidate.
    KernelDiscovery,
    /// Tree populated; the app accepts interaction.
    Ready,
    /// `closeEvent` — the worker is stopped, the alpm handle released.
    Shutdown,
}

/// The kernel catalog (the tree rows + their freshness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum CatalogState {
    /// No catalog loaded yet.
    Unloaded,
    /// Rows populated.
    Loaded,
    /// A transaction changed the package state: re-discovery is pending
    /// (`is_kernels_change_state`, `km-window.cpp:150-166`); the OK button
    /// stays disabled until the refresh completes (`init_kernels`,
    /// `km-window.cpp:361-375`).
    RefreshPending,
}

/// The transaction pipeline (install → remove → commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum TransactionState {
    /// Nothing executing.
    Idle,
    /// `install_packages`/`remove_packages` + `Kernel::install` expansion.
    Planning,
    /// pkexec polkit prompt.
    Authenticating,
    /// The worker thread runs the commit (`m_running == true`).
    Running,
    /// The commit finished; `changed` = the kernels-change-state flag.
    Complete { changed: bool },
    /// The commit failed.
    Failed,
}

/// The Configure window's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum ConfigurationState {
    /// Window closed.
    Closed,
    /// `on_configure`: QtConcurrent git refresh + progress dialog
    /// (`km-window.cpp:340-351`).
    Preparing,
    /// Options/Patches tabs visible.
    Editing,
}

/// The build pipeline (Configure window's Build kernel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum BuildState {
    /// No build in flight.
    Idle,
    /// `makepkg ... && touch .done-status` async (`conf-window.cpp:734`).
    Running,
    /// `.done-status` absent → the failure branch.
    Failed,
    /// `.done-status` present (`finished_proc:385`).
    Completed,
    /// The user said Yes to the install question; `sudo pacman -U` runs.
    Installing,
}

/// The sched-ext window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum ScxState {
    /// The window is hidden (the button may still be visible).
    Hidden,
    /// `on_schedext_config` → `m_sched_window->show()`.
    Visible,
}

/// The dialog overlay.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // component semantics documented above + docs/ARCHITECTURE.md
pub enum DialogsState {
    /// No dialog.
    None,
    /// The indeterminate progress dialog.
    Progress { message: String },
    /// A `QMessageBox::critical` error dialog.
    Error { message: String },
    /// A `QMessageBox::question` confirmation.
    Confirm { message: String },
}

/// Effects are explicit values; the UI/execution layer interprets them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // effect semantics documented in docs/ARCHITECTURE.md
pub enum Effect {
    /// Re-run kernel discovery (post-transaction refresh).
    RefreshKernels,
    /// Run the pacman transaction (install then remove phases).
    RunTransaction,
    /// Run the build (`makepkg ... && touch .done-status`).
    RunBuild { variant_dir: String },
    /// Install built artifacts (`sudo pacman -U <globs>`).
    InstallArtifacts,
    /// Authenticate (pkexec).
    Authenticate,
    /// The QtConcurrent `prepare_build_environment` + patches reset
    /// (`on_configure`, `km-window.cpp:347-350`).
    PrepareConfiguration,
    /// Toggle the sched-ext window (`on_schedext_config`).
    ToggleScxWindow,
    /// Close the window.
    Close,
    /// Show an error dialog.
    ShowError { message: String },
    /// Show a progress/status dialog.
    ShowProgress { message: String },
    /// Hide the progress dialog.
    HideProgress,
}

/// Semantic UI events (directive §7 style).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // event semantics documented in docs/ARCHITECTURE.md
pub enum AppEvent {
    Started,
    DiscoveryFinished,
    KernelToggled { row: usize },
    ExecuteRequested,
    TransactionFinished { changed: bool },
    TransactionFailed { message: String },
    ConfigureRequested,
    ConfigurePrepared,
    BuildFinished { success: bool },
    ArtifactsInstalled,
    ScxToggleRequested,
    CloseRequested,
}

/// The application model — orthogonal state components (Phase 8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // fields documented in docs/ARCHITECTURE.md
pub struct AppState {
    pub lifecycle: LifecycleState,
    pub catalog: CatalogState,
    pub selection: SelectionState,
    pub transaction: TransactionState,
    pub configuration: ConfigurationState,
    pub build: BuildState,
    pub scx: ScxState,
    pub dialogs: DialogsState,
    pub build_options: BuildOptions,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            lifecycle: LifecycleState::Startup,
            catalog: CatalogState::Unloaded,
            selection: SelectionState::default(),
            transaction: TransactionState::Idle,
            configuration: ConfigurationState::Closed,
            build: BuildState::Idle,
            scx: ScxState::Hidden,
            dialogs: DialogsState::None,
            build_options: BuildOptions::default(),
        }
    }
}

impl AppState {
    /// The derived coarse phase (docs/ARCHITECTURE.md §State machine table).
    /// Computed from the components; the phase enum is never stored.
    pub fn phase(&self) -> AppPhase {
        match self.lifecycle {
            LifecycleState::Startup => return AppPhase::Startup,
            LifecycleState::KernelDiscovery => return AppPhase::KernelDiscovery,
            LifecycleState::Shutdown => return AppPhase::Shutdown,
            LifecycleState::Ready => {}
        }
        match self.transaction {
            TransactionState::Planning => return AppPhase::TransactionPlanning,
            TransactionState::Authenticating => return AppPhase::Authenticating,
            TransactionState::Running => return AppPhase::TransactionRunning,
            TransactionState::Complete { changed } => {
                if changed {
                    return AppPhase::RefreshingPackageState;
                }
                return AppPhase::TransactionComplete;
            }
            TransactionState::Failed => return AppPhase::TransactionFailed,
            TransactionState::Idle => {}
        }
        match self.configuration {
            ConfigurationState::Preparing => return AppPhase::ConfigurationPreparation,
            ConfigurationState::Editing => {
                return match self.build {
                    BuildState::Idle => AppPhase::ConfigurationEditing,
                    BuildState::Running => AppPhase::BuildRunning,
                    BuildState::Failed => AppPhase::BuildFailed,
                    BuildState::Completed => AppPhase::BuildCompleted,
                    BuildState::Installing => AppPhase::ArtifactInstallation,
                };
            }
            ConfigurationState::Closed => {}
        }
        if self.scx == ScxState::Visible {
            return AppPhase::ScxConfiguration;
        }
        match self.catalog {
            CatalogState::Unloaded => AppPhase::KernelDiscovery,
            CatalogState::Loaded => {
                if self.selection.change_list().is_empty() {
                    AppPhase::Ready
                } else {
                    AppPhase::SelectionChanged
                }
            }
            CatalogState::RefreshPending => AppPhase::RefreshingPackageState,
        }
    }

    /// The oracle's OK-button enablement: enabled iff the change list is
    /// non-empty AND no transaction is running (`build_change_list`,
    /// `km-window.cpp:307-324`; the worker disables it at 125 and
    /// re-enables at 174 only when nothing changed).
    pub fn execute_enabled(&self) -> bool {
        self.transaction == TransactionState::Idle
            && !self.selection.change_list().is_empty()
            && self.catalog != CatalogState::RefreshPending
    }

    /// The oracle's `m_running` flag projection.
    pub fn transaction_in_progress(&self) -> bool {
        matches!(
            self.transaction,
            TransactionState::Planning
                | TransactionState::Authenticating
                | TransactionState::Running
        )
    }
}

/// Pure transition function. Returns the next state and the effects to run.
///
/// Semantics preserved from the oracle:
/// - `ExecuteRequested` while a transaction is in progress is ignored
///   (`on_execute`, `km-window.cpp:378-380`).
/// - After a transaction the UI is re-enabled only when nothing changed
///   (`km-window.cpp:174`); when kernels changed, discovery re-runs and the
///   OK button stays disabled until the refresh completes
///   (`init_kernels`, `km-window.cpp:361-375`).
/// - `ConfigureRequested` → the prepare flow (`km-window.cpp:340-351`).
/// - `BuildFinished` keys on `.done-status` presence (`conf-window.cpp:385`),
///   not the exit code (gap: the failure branch is `finished_proc`'s stderr).
pub fn transition(state: &AppState, event: AppEvent) -> (AppState, Vec<Effect>) {
    let mut next = state.clone();
    let mut effects = Vec::new();
    match event {
        AppEvent::Started => {
            next.lifecycle = LifecycleState::KernelDiscovery;
            effects.push(Effect::ShowProgress {
                message: "Please wait...\nInitializing kernels..".into(),
            });
        }
        AppEvent::DiscoveryFinished => {
            next.lifecycle = LifecycleState::Ready;
            next.catalog = CatalogState::Loaded;
            effects.push(Effect::HideProgress);
        }
        AppEvent::KernelToggled { row } => {
            crate::selection::toggle_row(&mut next.selection, row);
        }
        AppEvent::ExecuteRequested => {
            if state.transaction_in_progress() {
                // on_execute: `if (m_running.load()) return;`
                return (next, effects);
            }
            next.transaction = TransactionState::Planning;
            effects.push(Effect::Authenticate);
            effects.push(Effect::RunTransaction);
        }
        AppEvent::TransactionFinished { changed } => {
            next.transaction = TransactionState::Complete { changed };
            if changed {
                next.catalog = CatalogState::RefreshPending;
                effects.push(Effect::RefreshKernels);
                effects.push(Effect::ShowProgress {
                    message: "Please wait...\nInitializing kernels..".into(),
                });
            } else {
                next.catalog = CatalogState::Loaded;
                effects.push(Effect::HideProgress);
            }
        }
        AppEvent::TransactionFailed { message } => {
            next.transaction = TransactionState::Failed;
            effects.push(Effect::ShowError { message });
        }
        AppEvent::ConfigureRequested => {
            next.configuration = ConfigurationState::Preparing;
            effects.push(Effect::PrepareConfiguration);
            effects.push(Effect::ShowProgress {
                message: "Please wait...\nWe are preparing configuration window for you\ncloning PKGBUILDs..".into(),
            });
        }
        AppEvent::ConfigurePrepared => {
            next.configuration = ConfigurationState::Editing;
            effects.push(Effect::HideProgress);
        }
        AppEvent::BuildFinished { success } => {
            next.build = if success {
                BuildState::Completed
            } else {
                BuildState::Failed
            };
        }
        AppEvent::ArtifactsInstalled => {
            next.build = BuildState::Idle;
        }
        AppEvent::ScxToggleRequested => {
            next.scx = match state.scx {
                ScxState::Hidden => ScxState::Visible,
                ScxState::Visible => ScxState::Hidden,
            };
            effects.push(Effect::ToggleScxWindow);
        }
        AppEvent::CloseRequested => {
            next.lifecycle = LifecycleState::Shutdown;
            effects.push(Effect::Close);
        }
    }
    (next, effects)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn ready_state() -> AppState {
        let mut s = AppState::default();
        s.lifecycle = LifecycleState::Ready;
        s.catalog = CatalogState::Loaded;
        s
    }

    #[test]
    fn startup_enters_kernel_discovery() {
        let (s, fx) = transition(&AppState::default(), AppEvent::Started);
        assert_eq!(s.lifecycle, LifecycleState::KernelDiscovery);
        assert_eq!(s.phase(), AppPhase::KernelDiscovery);
        assert!(fx.iter().any(|e| matches!(e, Effect::ShowProgress { .. })));
    }

    #[test]
    fn execute_is_gated_by_in_progress() {
        let s = ready_state();
        let (s2, fx) = transition(&s, AppEvent::ExecuteRequested);
        assert!(s2.transaction_in_progress());
        assert_eq!(s2.transaction, TransactionState::Planning);
        assert_eq!(s2.phase(), AppPhase::TransactionPlanning);
        assert!(fx.contains(&Effect::RunTransaction));

        // second ExecuteRequested while running is ignored
        let (s3, fx3) = transition(&s2, AppEvent::ExecuteRequested);
        assert_eq!(s3, s2);
        assert!(fx3.is_empty());
    }

    #[test]
    fn transaction_completion_refreshes_when_changed() {
        let mut s = ready_state();
        s.transaction = TransactionState::Running;
        let (s2, fx) = transition(&s, AppEvent::TransactionFinished { changed: true });
        assert_eq!(s2.catalog, CatalogState::RefreshPending);
        assert_eq!(s2.phase(), AppPhase::RefreshingPackageState);
        assert!(!s2.transaction_in_progress());
        assert!(fx.contains(&Effect::RefreshKernels));

        let (s3, fx3) = transition(&s, AppEvent::TransactionFinished { changed: false });
        assert_eq!(s3.catalog, CatalogState::Loaded);
        assert_eq!(s3.phase(), AppPhase::TransactionComplete);
        assert!(fx3.contains(&Effect::HideProgress));
        assert!(!fx3.contains(&Effect::RefreshKernels));
    }

    #[test]
    fn close_requests_shutdown() {
        let (s, fx) = transition(&AppState::default(), AppEvent::CloseRequested);
        assert_eq!(s.lifecycle, LifecycleState::Shutdown);
        assert_eq!(s.phase(), AppPhase::Shutdown);
        assert_eq!(fx, vec![Effect::Close]);
    }

    #[test]
    fn orthogonal_state_is_partially_independent() {
        // catalog=Loaded, selection=Dirty, build=Running, scx=Visible,
        // dialog=Progress — the Cartesian product a single phase enum could
        // not express (the Phase-8-start note).
        let mut s = ready_state();
        s.selection.rows.push(crate::selection::KernelRow {
            raw: "cachyos/linux-cachyos".into(),
            name: "linux-cachyos".into(),
            installed: false,
            immutable: false,
            update_available: false,
            checked: true,
        });
        s.build = BuildState::Running;
        s.scx = ScxState::Visible;
        s.dialogs = DialogsState::Progress {
            message: "Please wait...\nInitializing kernels..".into(),
        };
        assert_eq!(s.catalog, CatalogState::Loaded);
        assert_eq!(s.build, BuildState::Running);
        assert_eq!(s.scx, ScxState::Visible);
        assert!(s.execute_enabled()); // transaction idle + dirty selection
        assert!(s.selection.change_list().len() == 1);
    }

    #[test]
    fn execute_enabled_follows_change_list_and_transaction() {
        let s = ready_state();
        assert!(!s.execute_enabled()); // empty change list

        let mut dirty = ready_state();
        dirty.selection.rows.push(crate::selection::KernelRow {
            raw: "cachyos/linux-cachyos".into(),
            name: "linux-cachyos".into(),
            installed: false,
            immutable: false,
            update_available: false,
            checked: true,
        });
        assert!(dirty.execute_enabled());

        let mut running = dirty.clone();
        running.transaction = TransactionState::Running;
        assert!(!running.execute_enabled());

        let mut refreshing = dirty.clone();
        refreshing.catalog = CatalogState::RefreshPending;
        assert!(!refreshing.execute_enabled());
    }

    #[test]
    fn configure_flow_prepares_then_edits() {
        let s = ready_state();
        let (s2, fx) = transition(&s, AppEvent::ConfigureRequested);
        assert_eq!(s2.configuration, ConfigurationState::Preparing);
        assert_eq!(s2.phase(), AppPhase::ConfigurationPreparation);
        assert!(fx.contains(&Effect::PrepareConfiguration));

        let (s3, fx3) = transition(&s2, AppEvent::ConfigurePrepared);
        assert_eq!(s3.configuration, ConfigurationState::Editing);
        assert_eq!(s3.phase(), AppPhase::ConfigurationEditing);
        assert!(fx3.contains(&Effect::HideProgress));
    }

    #[test]
    fn build_finished_keys_on_done_status() {
        let mut s = ready_state();
        s.configuration = ConfigurationState::Editing;
        let (s2, _) = transition(&s, AppEvent::BuildFinished { success: true });
        assert_eq!(s2.build, BuildState::Completed);
        assert_eq!(s2.phase(), AppPhase::BuildCompleted);

        let (s3, _) = transition(&s, AppEvent::BuildFinished { success: false });
        assert_eq!(s3.build, BuildState::Failed);
        assert_eq!(s3.phase(), AppPhase::BuildFailed);
    }

    #[test]
    fn scx_toggle_is_independent() {
        let s = ready_state();
        let (s2, fx) = transition(&s, AppEvent::ScxToggleRequested);
        assert_eq!(s2.scx, ScxState::Visible);
        assert_eq!(s2.phase(), AppPhase::ScxConfiguration);
        assert!(fx.contains(&Effect::ToggleScxWindow));
        let (s3, _) = transition(&s2, AppEvent::ScxToggleRequested);
        assert_eq!(s3.scx, ScxState::Hidden);
    }
}
