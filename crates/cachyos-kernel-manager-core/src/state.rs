//! Application state machine.
//!
//! The oracle's implicit state (constructor → worker thread → refresh) is
//! made explicit here as pure transitions `State + Event → (State, Effects)`
//! so it can be replayed, tested, and shrunk (directive §8).
//!
//! The phase list mirrors the oracle's observable lifecycle
//! (`km-window.cpp`, `conf-window.cpp`).

use crate::options::BuildOptions;
use crate::selection::SelectionState;

/// Application phases (directive §8 list, mapped to oracle evidence in
/// docs/ARCHITECTURE.md).
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
    ConfigureRequested,
    ConfigurePrepared,
    TransactionFinished { changed: bool },
    TransactionFailed { message: String },
    BuildFinished { success: bool },
    ArtifactsInstalled,
    CloseRequested,
}

/// The application model. Minimal now; grows with each phase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // fields documented in docs/ARCHITECTURE.md
pub struct AppState {
    pub phase: AppPhase,
    pub selection: SelectionState,
    pub build: BuildOptions,
    /// True while a transaction is executing (the oracle's `m_running`).
    pub transaction_in_progress: bool,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            phase: AppPhase::Startup,
            selection: SelectionState::default(),
            build: BuildOptions::default(),
            transaction_in_progress: false,
        }
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
pub fn transition(state: &AppState, event: AppEvent) -> (AppState, Vec<Effect>) {
    let mut next = state.clone();
    let mut effects = Vec::new();
    match event {
        AppEvent::Started => {
            next.phase = AppPhase::KernelDiscovery;
            effects.push(Effect::ShowProgress {
                message: "Please wait...\nInitializing kernels..".into(),
            });
        }
        AppEvent::DiscoveryFinished => {
            next.phase = AppPhase::Ready;
            effects.push(Effect::HideProgress);
        }
        AppEvent::KernelToggled { row } => {
            crate::selection::toggle_row(&mut next.selection, row);
            next.phase = AppPhase::SelectionChanged;
        }
        AppEvent::ExecuteRequested => {
            if state.transaction_in_progress {
                // on_execute: `if (m_running.load()) return;`
                return (next, effects);
            }
            next.transaction_in_progress = true;
            next.phase = AppPhase::TransactionPlanning;
            effects.push(Effect::Authenticate);
            effects.push(Effect::RunTransaction);
        }
        AppEvent::ConfigureRequested => {
            next.phase = AppPhase::ConfigurationPreparation;
            effects.push(Effect::ShowProgress {
                message: "Please wait...\nWe are preparing configuration window for you\ncloning PKGBUILDs..".into(),
            });
        }
        AppEvent::ConfigurePrepared => {
            next.phase = AppPhase::ConfigurationEditing;
            effects.push(Effect::HideProgress);
        }
        AppEvent::TransactionFinished { changed } => {
            next.transaction_in_progress = false;
            if changed {
                next.phase = AppPhase::RefreshingPackageState;
                effects.push(Effect::RefreshKernels);
                effects.push(Effect::ShowProgress {
                    message: "Please wait...\nInitializing kernels..".into(),
                });
            } else {
                next.phase = AppPhase::Ready;
                effects.push(Effect::HideProgress);
            }
        }
        AppEvent::TransactionFailed { message } => {
            next.transaction_in_progress = false;
            next.phase = AppPhase::TransactionFailed;
            effects.push(Effect::ShowError { message });
        }
        AppEvent::BuildFinished { success } => {
            next.phase = if success {
                AppPhase::BuildCompleted
            } else {
                AppPhase::BuildFailed
            };
        }
        AppEvent::ArtifactsInstalled => {
            next.phase = AppPhase::ConfigurationEditing;
        }
        AppEvent::CloseRequested => {
            next.phase = AppPhase::Shutdown;
            effects.push(Effect::Close);
        }
    }
    (next, effects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_is_gated_by_in_progress() {
        let s = AppState {
            phase: AppPhase::Ready,
            ..Default::default()
        };
        let (s2, fx) = transition(&s, AppEvent::ExecuteRequested);
        assert!(s2.transaction_in_progress);
        assert_eq!(s2.phase, AppPhase::TransactionPlanning);
        assert!(fx.contains(&Effect::RunTransaction));

        // second ExecuteRequested while running is ignored
        let (s3, fx3) = transition(&s2, AppEvent::ExecuteRequested);
        assert_eq!(s3.phase, s2.phase);
        assert!(fx3.is_empty());
        assert!(s3.transaction_in_progress);
    }

    #[test]
    fn transaction_completion_refreshes_when_changed() {
        let s = AppState {
            phase: AppPhase::TransactionRunning,
            transaction_in_progress: true,
            ..Default::default()
        };
        let (s2, fx) = transition(&s, AppEvent::TransactionFinished { changed: true });
        assert_eq!(s2.phase, AppPhase::RefreshingPackageState);
        assert!(!s2.transaction_in_progress);
        assert!(fx.contains(&Effect::RefreshKernels));

        let (s3, fx3) = transition(&s, AppEvent::TransactionFinished { changed: false });
        assert_eq!(s3.phase, AppPhase::Ready);
        assert!(fx3.contains(&Effect::HideProgress));
        assert!(!fx3.contains(&Effect::RefreshKernels));
    }

    #[test]
    fn close_requests_shutdown() {
        let (s, fx) = transition(&AppState::default(), AppEvent::CloseRequested);
        assert_eq!(s.phase, AppPhase::Shutdown);
        assert_eq!(fx, vec![Effect::Close]);
    }
}
