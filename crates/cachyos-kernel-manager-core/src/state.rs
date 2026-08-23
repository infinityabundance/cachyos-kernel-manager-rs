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
    /// `.done-status` present AND the "install build packages?" question is
    /// up (the oracle's finished_proc success branch, `conf-window.cpp:390`;
    /// m_running was ALREADY cleared at the start of finished_proc, so the
    /// user can retry the moment the question is answered).
    AwaitingInstallDecision,
    /// The user said Yes to the install question; `sudo pacman -U` runs.
    Installing,
}

impl BuildState {
    /// The oracle's `m_running` projection for the Configure window's
    /// on_execute guard (`conf-window.cpp:696-701`): in-flight means a
    /// build is actually executing OR the install question/install is up.
    /// OUTCOME states (Failed) are NOT in-flight — the oracle clears
    /// `m_running` at the START of `finished_proc`, so a failed build (and
    /// a success after the question is answered No) is IMMEDIATELY
    /// retryable (audit P0: the old guard used outcome states as the
    /// m_running projection, soft-locking the Build button).
    pub fn in_flight(self) -> bool {
        matches!(
            self,
            BuildState::Running | BuildState::AwaitingInstallDecision | BuildState::Installing
        )
    }
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
    /// Toggle the sched-ext window (`on_schedext_config` — the oracle's
    /// handler is `m_sched_window->show()`, always a SHOW).
    ShowScxWindow,
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
    KernelToggled {
        row: usize,
    },
    ExecuteRequested,
    /// The worker started the commit run (the phase projection shows
    /// `TransactionRunning` for the real work, not a parked Planning).
    TransactionStarted,
    TransactionFinished {
        changed: bool,
    },
    TransactionFailed {
        message: String,
    },
    /// The failure error dialog was acknowledged — the transaction returns to
    /// Idle (the oracle's `m_running` releases after the worker's finished
    /// path, after the message box).
    TransactionErrorAcknowledged,
    ConfigureRequested,
    ConfigurePrepared,
    BuildRequested,
    BuildFinished {
        success: bool,
    },
    InstallArtifactsRequested,
    /// The user answered No to the "install build packages?" question — the
    /// build flow is over and the build returns to Idle (retryable).
    InstallDeclined,
    ArtifactsInstalled,
    /// The sched-ext button (`on_schedext_config`, km-window.cpp:387-388):
    /// the oracle ALWAYS SHOWS the window — `m_sched_window->show()` never
    /// hides it. The old `ScxToggleRequested` flipped Visible↔Hidden, so
    /// clicking the button while the window was open HID it instead of
    /// raising it (audit P2). Closing the window is the separate
    /// `ScxWindowClosed` event.
    ScxShowRequested,
    /// Close the MAIN window (the oracle's closeEvent exits the app).
    CloseRequested,
    /// The Configure window's Cancel/Close: closes the CONFIGURE window,
    /// NOT the app (the oracle's `on_cancel`, conf-window.cpp:565-570).
    ConfigurationCancelRequested,
    ConfigurationCloseRequested,
    /// The sched-ext window's close: hides the SCX window, NOT the app
    /// (the oracle's schedext-window closeEvent just closes the window;
    /// the main window stays alive and can reopen it). Distinct from
    /// `ScxShowRequested` (the button: show + re-init).
    ScxWindowClosed,
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
                    BuildState::AwaitingInstallDecision => AppPhase::BuildCompleted,
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
            // The oracle discovers kernels SYNCHRONOUSLY at construction
            // (`km-window.hpp:141`: `m_kernels = Kernel::get_kernels(m_handle)`
            // runs before the window shows). The candidate runs the same
            // discovery as a background task — WITHOUT this effect the app
            // sat on the "Initializing kernels.." dialog forever with an
            // empty catalog (the observed VM hang: phase stuck in
            // KernelDiscovery, no CatalogLoaded ever arriving).
            effects.push(Effect::RefreshKernels);
        }
        AppEvent::DiscoveryFinished => {
            next.lifecycle = LifecycleState::Ready;
            next.catalog = CatalogState::Loaded;
            effects.push(Effect::HideProgress);
            // a transaction that CHANGED the package state parked the
            // transaction in Complete + RefreshPending; this discovery IS the
            // post-transaction refresh — the transaction returns to Idle so
            // the OK button re-enables (km-window.cpp:174 / the refresh
            // completion path). The STARTUP discovery (catalog == Unloaded)
            // leaves Idle untouched.
            if state.catalog == CatalogState::RefreshPending {
                next.transaction = TransactionState::Idle;
            }
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
        // The worker actually started the commit run (the UI sends this when
        // the terminal-helper spawns) — the phase projection shows the real
        // work instead of parking on Planning for the whole run.
        AppEvent::TransactionStarted => {
            next.transaction = TransactionState::Running;
        }
        AppEvent::TransactionFinished { changed } => {
            if changed {
                next.transaction = TransactionState::Complete { changed };
                next.catalog = CatalogState::RefreshPending;
                effects.push(Effect::RefreshKernels);
                effects.push(Effect::ShowProgress {
                    message: "Please wait...\nInitializing kernels..".into(),
                });
            } else {
                // nothing changed: the oracle re-enables the OK button
                // immediately (km-window.cpp:174) — back to Idle, no refresh.
                next.transaction = TransactionState::Idle;
                next.catalog = CatalogState::Loaded;
                effects.push(Effect::HideProgress);
            }
        }
        AppEvent::TransactionFailed { message } => {
            next.transaction = TransactionState::Failed;
            effects.push(Effect::ShowError { message });
        }
        // the error dialog was acknowledged: the worker is done, the OK
        // button re-enables (the oracle's m_running is released when the
        // worker's finished path ends, after the message box).
        AppEvent::TransactionErrorAcknowledged => {
            next.transaction = TransactionState::Idle;
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
        AppEvent::BuildRequested => {
            if state.build.in_flight() {
                // the Configure window's on_execute guard: `if (m_running)
                // return;` (conf-window.cpp:696-701) — a double-click/double-
                // press must NEVER spawn a concurrent makepkg run (they race
                // on .done-status and confuse finished_proc). m_running is
                // cleared at the START of finished_proc, so only in-flight
                // states block; a FAILED build and a success whose question
                // was answered No are immediately retryable (audit P0).
                return (next, effects);
            }
            next.build = BuildState::Running;
            // the Configure window's on_execute (`conf-window.cpp:696-735`):
            // makepkg runs in the variant's PKGBUILD dir under the cache
            // root; the UI layer (platform crate) joins the full path.
            let variant_dir = next.build_options.variant.dir_name().to_string();
            effects.push(Effect::RunBuild { variant_dir });
        }
        AppEvent::InstallArtifactsRequested => {
            next.build = BuildState::Installing;
            effects.push(Effect::InstallArtifacts);
        }
        AppEvent::BuildFinished { success } => {
            next.build = if success {
                // the worker found .done-status; the install question is up
                // (finished_proc:385-390 — m_running already cleared, so the
                // user can retry as soon as the question is answered)
                BuildState::AwaitingInstallDecision
            } else {
                BuildState::Failed
            };
        }
        // the user answered No to "install build packages?": the flow is
        // over, the build returns to Idle (immediately retryable).
        AppEvent::InstallDeclined => {
            next.build = BuildState::Idle;
        }
        AppEvent::ArtifactsInstalled => {
            next.build = BuildState::Idle;
        }
        AppEvent::ScxShowRequested => {
            // the oracle's `on_schedext_config` is `m_sched_window->show()`
            // (km-window.cpp:387-388) — ALWAYS show, never hide (audit P2).
            next.scx = ScxState::Visible;
            effects.push(Effect::ShowScxWindow);
        }
        AppEvent::CloseRequested => {
            next.lifecycle = LifecycleState::Shutdown;
            effects.push(Effect::Close);
        }
        AppEvent::ScxWindowClosed => {
            // the window closed: hide it; the UI layer re-hides the Slint
            // window on sync. No ShowScxWindow effect (no re-init — the
            // oracle's closeEvent has no side effects).
            next.scx = ScxState::Hidden;
        }
        // the Configure window's Cancel/Close dismisses ONLY the Configure
        // window (`on_cancel` conf-window.cpp:565-570: hide + reset); the
        // main window stays alive
        AppEvent::ConfigurationCancelRequested | AppEvent::ConfigurationCloseRequested => {
            next.configuration = ConfigurationState::Closed;
            effects.push(Effect::HideProgress);
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
        // the oracle discovers at construction; the startup MUST spawn the
        // discovery task (regression: without it the app never loads a
        // catalog and the kernel list can never appear)
        assert!(fx.contains(&Effect::RefreshKernels));
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
        // NOTHING changed: the transaction returns to Idle immediately — the
        // OK button re-enables (km-window.cpp:174). Regression: it used to
        // park in Complete forever, soft-locking Execute for the process
        // lifetime.
        assert_eq!(s3.transaction, TransactionState::Idle);
        assert!(fx3.contains(&Effect::HideProgress));
        assert!(!fx3.contains(&Effect::RefreshKernels));
    }

    #[test]
    fn changed_transaction_returns_to_idle_after_the_refresh_discovery() {
        // changed:true -> Complete + RefreshPending; the post-transaction
        // DiscoveryFinished IS the refresh completion -> Idle (the OK button
        // re-enables after the tree is rebuilt).
        let mut s = ready_state();
        s.transaction = TransactionState::Running;
        let (s2, _) = transition(&s, AppEvent::TransactionFinished { changed: true });
        assert_eq!(s2.transaction, TransactionState::Complete { changed: true });
        assert_eq!(s2.catalog, CatalogState::RefreshPending);
        let (s3, _) = transition(&s2, AppEvent::DiscoveryFinished);
        assert_eq!(s3.transaction, TransactionState::Idle);
        assert_eq!(s3.catalog, CatalogState::Loaded);
        assert!(s3.execute_enabled() || s3.selection.change_list().is_empty());
        // the STARTUP discovery (catalog Unloaded) leaves Idle untouched
        let (s4, _) = transition(&AppState::default(), AppEvent::DiscoveryFinished);
        assert_eq!(s4.transaction, TransactionState::Idle);
    }

    #[test]
    fn transaction_error_acknowledged_returns_to_idle() {
        // failed -> the error dialog; acknowledging it releases the worker's
        // m_running equivalent and re-enables the OK button.
        let s = ready_state();
        let (s2, fx) = transition(
            &s,
            AppEvent::TransactionFailed {
                message: "alpm init failed".into(),
            },
        );
        assert_eq!(s2.transaction, TransactionState::Failed);
        assert_eq!(s2.phase(), AppPhase::TransactionFailed);
        assert!(fx.contains(&Effect::ShowError {
            message: "alpm init failed".into()
        }));
        let (s3, _) = transition(&s2, AppEvent::TransactionErrorAcknowledged);
        assert_eq!(s3.transaction, TransactionState::Idle);
        assert!(s3.execute_enabled() || s3.selection.change_list().is_empty());
    }

    #[test]
    fn transaction_started_enters_running() {
        let mut s = ready_state();
        s.transaction = TransactionState::Planning;
        let (s2, fx) = transition(&s, AppEvent::TransactionStarted);
        assert_eq!(s2.transaction, TransactionState::Running);
        assert_eq!(s2.phase(), AppPhase::TransactionRunning);
        assert!(s2.transaction_in_progress());
        assert!(fx.is_empty());
    }

    #[test]
    fn build_requested_is_gated_while_a_build_or_install_runs() {
        // conf-window.cpp:696-701 `if (m_running) return;` — a second Execute
        // while the build OR the install question/install is up is a
        // complete no-op (a double-click must never spawn concurrent makepkg
        // runs). m_running is cleared at the START of finished_proc, so
        // OUTCOME states are NOT in-flight: a failed build is immediately
        // retryable (audit P0: the old guard blocked everything but Idle,
        // soft-locking the Build button after the first build).
        for blocked in [
            BuildState::Running,
            BuildState::AwaitingInstallDecision,
            BuildState::Installing,
        ] {
            let mut s = ready_state();
            s.configuration = ConfigurationState::Editing;
            s.build = blocked;
            let (s2, fx) = transition(&s, AppEvent::BuildRequested);
            assert_eq!(s2, s, "build must stay {blocked:?} under a second Execute");
            assert!(fx.is_empty());
        }
        // Failed and Idle are retryable
        for retryable in [BuildState::Idle, BuildState::Failed] {
            let mut s = ready_state();
            s.configuration = ConfigurationState::Editing;
            s.build = retryable;
            let (s2, fx) = transition(&s, AppEvent::BuildRequested);
            assert_eq!(s2.build, BuildState::Running);
            assert!(fx.iter().any(
                |e| matches!(e, Effect::RunBuild { variant_dir } if variant_dir == "linux-cachyos")
            ));
        }
    }

    #[test]
    fn close_requests_shutdown() {
        let (s, fx) = transition(&AppState::default(), AppEvent::CloseRequested);
        assert_eq!(s.lifecycle, LifecycleState::Shutdown);
        assert_eq!(s.phase(), AppPhase::Shutdown);
        assert_eq!(fx, vec![Effect::Close]);
    }

    #[test]
    fn configure_cancel_closes_only_the_configure_window() {
        // the review seam: Configure->Cancel must NOT exit the app
        let s = ready_state();
        let (s2, _) = transition(&s, AppEvent::ConfigureRequested);
        assert_eq!(s2.configuration, ConfigurationState::Preparing);
        let (s3, _) = transition(&s2, AppEvent::ConfigurationCancelRequested);
        assert_eq!(s3.configuration, ConfigurationState::Closed);
        assert_eq!(s3.lifecycle, LifecycleState::Ready); // the app is alive
                                                         // while the main window's Close still exits
        let (s4, fx) = transition(&ready_state(), AppEvent::CloseRequested);
        assert_eq!(s4.lifecycle, LifecycleState::Shutdown);
        assert!(fx.iter().any(|e| matches!(e, Effect::Close)));
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
        assert_eq!(s2.build, BuildState::AwaitingInstallDecision);
        assert_eq!(s2.phase(), AppPhase::BuildCompleted);

        let (s3, _) = transition(&s, AppEvent::BuildFinished { success: false });
        assert_eq!(s3.build, BuildState::Failed);
        assert_eq!(s3.phase(), AppPhase::BuildFailed);
    }

    #[test]
    fn build_and_install_artifacts_flow() {
        let mut s = ready_state();
        s.configuration = ConfigurationState::Editing;
        // Build kernel: on_execute (conf-window.cpp:696-735) — the variant's
        // dir name feeds the RunBuild effect.
        let (s2, fx) = transition(&s, AppEvent::BuildRequested);
        assert_eq!(s2.build, BuildState::Running);
        assert_eq!(s2.phase(), AppPhase::BuildRunning);
        assert!(fx.iter().any(
            |e| matches!(e, Effect::RunBuild { variant_dir } if variant_dir == "linux-cachyos")
        ));

        // finished_proc found .done-status -> Completed; the install question
        // -> InstallArtifactsRequested -> Installing.
        let (s3, _) = transition(&s2, AppEvent::BuildFinished { success: true });
        assert_eq!(s3.build, BuildState::AwaitingInstallDecision);
        let (s4, fx4) = transition(&s3, AppEvent::InstallArtifactsRequested);
        assert_eq!(s4.build, BuildState::Installing);
        assert_eq!(s4.phase(), AppPhase::ArtifactInstallation);
        assert!(fx4.contains(&Effect::InstallArtifacts));
        let (s5, _) = transition(&s4, AppEvent::ArtifactsInstalled);
        assert_eq!(s5.build, BuildState::Idle);
        // answering No is immediately retryable (m_running cleared at the
        // start of finished_proc)
        let (s6, _) = transition(&s3, AppEvent::InstallDeclined);
        assert_eq!(s6.build, BuildState::Idle);
        let (s7, fx7) = transition(&s6, AppEvent::BuildRequested);
        assert_eq!(s7.build, BuildState::Running);
        assert!(fx7.iter().any(|e| matches!(e, Effect::RunBuild { .. })));
    }

    #[test]
    fn scx_show_always_shows() {
        // audit P2: the oracle's on_schedext_config is
        // `m_sched_window->show()` — the button ALWAYS shows, never hides.
        let s = ready_state();
        let (s2, fx) = transition(&s, AppEvent::ScxShowRequested);
        assert_eq!(s2.scx, ScxState::Visible);
        assert_eq!(s2.phase(), AppPhase::ScxConfiguration);
        assert!(fx.contains(&Effect::ShowScxWindow));
        // a second click while visible STAYS visible (show() again) — the
        // old toggle would have hidden it
        let (s3, fx3) = transition(&s2, AppEvent::ScxShowRequested);
        assert_eq!(s3.scx, ScxState::Visible);
        assert!(fx3.contains(&Effect::ShowScxWindow));
    }

    #[test]
    fn scx_window_close_hides_only_the_scx_window() {
        let s = ready_state();
        let (s2, _) = transition(&s, AppEvent::ScxShowRequested);
        assert_eq!(s2.scx, ScxState::Visible);
        let (s3, fx) = transition(&s2, AppEvent::ScxWindowClosed);
        // hidden, the app alive, NO ShowScxWindow (no re-init)
        assert_eq!(s3.scx, ScxState::Hidden);
        assert_eq!(s3.lifecycle, LifecycleState::Ready);
        assert!(!fx.contains(&Effect::ShowScxWindow));
        assert!(!fx.contains(&Effect::Close));
    }
}
