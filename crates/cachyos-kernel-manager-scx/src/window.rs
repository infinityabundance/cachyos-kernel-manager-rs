//! The main-window + SchedExtWindow integration decisions
//! (`km-window.cpp:185-188`, `schedext-window-internal.cpp`).
//!
//! - the sched-ext button is hidden unless `/sys/kernel/sched_ext/state`
//!   exists;
//! - the SchedExtWindow init sequence (config init, supported schedulers,
//!   initial values, the profile-visibility rule);
//! - the window's apply/disable button flows (the dialogs on failure).

use crate::config::{flags_for_mode, mode_from_label, SchedConfig, SchedMode, SupportedSched};
use serde::{Deserialize, Serialize};

/// `km-window.cpp:185-188`: hide the sched-ext button unless the state
/// file exists.
pub fn main_window_schedext_visible(state_file_exists: bool) -> bool {
    state_file_exists
}

/// `on_sched_changed` (`schedext-window-internal.cpp:250-264`): the profile
/// selection UI is visible ONLY for scx_bpfland and scx_lavd — the only
/// schedulers with preset profiles.
pub fn profile_ui_visible(scheduler: &str) -> bool {
    scheduler == "scx_bpfland" || scheduler == "scx_lavd"
}

/// The profile combo items (`schedext-window-internal.cpp:153-157`).
pub const PROFILE_ITEMS: [&str; 5] = ["Auto", "Gaming", "Powersave", "Lowlatency", "Server"];

/// The window-init decision (`schedext-window-internal.cpp:120-190`), as a
/// pure trace. Inputs: the config init result, the supported-schedulers
/// D-Bus call result, the parsed config, and the sysfs current-scheduler
/// readback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInitInput {
    /// `Config::init_config` failed (a malformed config file; a MISSING
    /// file is Ok(default) — `scx_loader_config.rs init_config`).
    pub config_init_failed: bool,
    /// The `get_supported_scheds` D-Bus call outcome.
    pub supported_scheds: Result<Vec<String>, String>,
    /// The config (parsed, or the default when the file is absent).
    pub config: SchedConfig,
    /// The sysfs readback label (`get_current_scheduler`).
    pub current_scheduler_label: String,
}

/// One init decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InitStep {
    /// `QMessageBox::critical(..., "Cannot initialize scx_loader configuration")`
    /// — the window stops (config init failed).
    CriticalConfigInit,
    /// The supported-schedulers combo items (the D-Bus list, in order).
    SchedulerCombo { items: Vec<String> },
    /// `QMessageBox::critical(... "Cannot get information from scx_loader!\nIs
    /// it working?\nThis is needed for the app to work properly")` — the
    /// scheduler-management widgets are HIDDEN and the init stops.
    CriticalNoLoader,
    /// The initial scheduler combo selection (`config.default_sched`).
    InitialScheduler { scheduler: Option<String> },
    /// The profile combo items (always the 5 profiles).
    ProfileCombo,
    /// The initial profile selection (`config.default_mode` index).
    InitialProfile { mode: Option<SchedMode> },
    /// The running-scheduler label (`get_current_scheduler`).
    CurrentSchedulerLabel { label: String },
    /// `on_sched_changed` runs at init for the initial scheduler: the
    /// profile-visibility decision.
    ProfileVisibility { visible: bool },
    /// The flags text `on_sched_profile_changed` renders for the initial
    /// (scheduler, mode).
    InitialFlags { text: String },
}

/// The init sequence (`schedext-window-internal.cpp:120-190`).
pub fn window_init(input: &WindowInitInput) -> Vec<InitStep> {
    let mut steps = Vec::new();

    // 1. config init (`scx_loader_config.rs init_config_file`): a parse
    //    failure -> critical dialog and the window STOPS (empty). A missing
    //    file is Ok(default config).
    if input.config_init_failed {
        steps.push(InitStep::CriticalConfigInit);
        return steps;
    }

    // 2. supported schedulers: failure -> critical + hide the scheduler-
    //    management widgets + stop.
    match &input.supported_scheds {
        Ok(items) => {
            steps.push(InitStep::SchedulerCombo {
                items: items.clone(),
            });
            steps.push(InitStep::InitialScheduler {
                scheduler: input.config.default_sched.map(|s| s.name().to_string()),
            });
            steps.push(InitStep::ProfileCombo);
            steps.push(InitStep::InitialProfile {
                mode: input.config.default_mode,
            });
            steps.push(InitStep::CurrentSchedulerLabel {
                label: input.current_scheduler_label.clone(),
            });
            let initial_sched = input
                .config
                .default_sched
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            steps.push(InitStep::ProfileVisibility {
                visible: profile_ui_visible(&initial_sched),
            });
            let initial_mode = input.config.default_mode.unwrap_or(SchedMode::Auto);
            let flags = flags_for_mode(
                &input.config,
                initial_sched.parse().unwrap_or(SupportedSched::Bpfland),
                initial_mode,
            );
            steps.push(InitStep::InitialFlags {
                text: flags.join(" "),
            });
        }
        Err(_) => {
            steps.push(InitStep::CriticalNoLoader);
        }
    }
    steps
}

/// The profile-change flags render (`on_sched_profile_changed`,
/// `schedext-window-internal.cpp:199-214`): the mode's flags for the
/// selected scheduler, space-joined.
pub fn flags_text(config: &SchedConfig, scheduler: &str, profile_label: &str) -> String {
    let mode = mode_from_label(profile_label);
    let sched = scheduler.parse().unwrap_or(SupportedSched::Bpfland);
    flags_for_mode(config, sched, mode).join(" ")
}

/// The apply-button flow (`on_apply`, `schedext-window-internal.cpp:266-282`):
/// the mode from the profile label, the trimmed extra flags, and the
/// failure dialog text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyUiDecision {
    pub scx_mode: SchedMode,
    pub extra_flags: String,
    /// The failure dialog text (only when the apply call fails).
    pub critical: Option<String>,
}

pub fn apply_ui_decision(
    scheduler: &str,
    profile_label: &str,
    flags_text_input: &str,
    apply_succeeded: bool,
) -> ApplyUiDecision {
    ApplyUiDecision {
        scx_mode: mode_from_label(profile_label),
        extra_flags: flags_text_input.trim().to_string(),
        critical: if apply_succeeded {
            None
        } else {
            Some(format!(
                "Cannot set default scx scheduler with mode! Scheduler {} with mode {}",
                scheduler, profile_label
            ))
        },
    }
}

/// The disable-button flow (`on_disable`, `schedext-window-internal.cpp:181-191`):
/// the failure dialog text.
pub fn disable_ui_decision(disable_succeeded: bool) -> Option<String> {
    if disable_succeeded {
        None
    } else {
        Some("Cannot disable scx_loader".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;

    #[test]
    fn button_visibility_follows_state_file() {
        assert!(main_window_schedext_visible(true));
        assert!(!main_window_schedext_visible(false));
    }

    #[test]
    fn profile_ui_only_for_bpfland_and_lavd() {
        assert!(profile_ui_visible("scx_bpfland"));
        assert!(profile_ui_visible("scx_lavd"));
        assert!(!profile_ui_visible("scx_rusty"));
        assert!(!profile_ui_visible("scx_flash"));
    }

    #[test]
    fn init_with_loader_ok_populates_everything() {
        let mut config = default_config();
        config.default_sched = Some(SupportedSched::Bpfland);
        config.default_mode = Some(SchedMode::Gaming);
        let steps = window_init(&WindowInitInput {
            config_init_failed: false,
            supported_scheds: Ok(vec!["scx_bpfland".into(), "scx_lavd".into()]),
            config,
            current_scheduler_label: "scx_bpfland".into(),
        });
        assert!(steps.contains(&InitStep::SchedulerCombo {
            items: vec!["scx_bpfland".into(), "scx_lavd".into()]
        }));
        assert!(steps.contains(&InitStep::InitialScheduler {
            scheduler: Some("scx_bpfland".into())
        }));
        assert!(steps.contains(&InitStep::InitialProfile {
            mode: Some(SchedMode::Gaming)
        }));
        assert!(steps.contains(&InitStep::CurrentSchedulerLabel {
            label: "scx_bpfland".into()
        }));
        assert!(steps.contains(&InitStep::ProfileVisibility { visible: true }));
        // gaming flags for bpfland
        assert!(steps.contains(&InitStep::InitialFlags {
            text: "-m performance".into()
        }));
    }

    #[test]
    fn init_without_loader_shows_critical_and_stops() {
        let steps = window_init(&WindowInitInput {
            config_init_failed: false,
            supported_scheds: Err("no loader".into()),
            config: default_config(),
            current_scheduler_label: "unknown".into(),
        });
        assert_eq!(steps, vec![InitStep::CriticalNoLoader]);
    }

    #[test]
    fn init_with_bad_config_stops_immediately() {
        let steps = window_init(&WindowInitInput {
            config_init_failed: true,
            supported_scheds: Ok(vec!["scx_bpfland".into()]),
            config: default_config(),
            current_scheduler_label: "unknown".into(),
        });
        assert_eq!(steps, vec![InitStep::CriticalConfigInit]);
    }

    #[test]
    fn flags_text_renders_mode_flags() {
        let config = default_config();
        assert_eq!(
            flags_text(&config, "scx_bpfland", "Gaming"),
            "-m performance"
        );
        assert_eq!(flags_text(&config, "scx_rusty", "Gaming"), "");
    }

    #[test]
    fn apply_ui_decision_builds_dialog() {
        let ok = apply_ui_decision("scx_bpfland", "Gaming", "  -m powersave  ", true);
        assert_eq!(ok.extra_flags, "-m powersave");
        assert_eq!(ok.scx_mode, SchedMode::Gaming);
        assert_eq!(ok.critical, None);
        let fail = apply_ui_decision("scx_bpfland", "Gaming", "", false);
        assert_eq!(
            fail.critical.as_deref(),
            Some("Cannot set default scx scheduler with mode! Scheduler scx_bpfland with mode Gaming")
        );
        assert_eq!(
            disable_ui_decision(false).as_deref(),
            Some("Cannot disable scx_loader")
        );
        assert_eq!(disable_ui_decision(true), None);
    }
}
