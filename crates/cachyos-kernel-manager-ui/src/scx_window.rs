//! The sched-ext window's semantic model — the Slint rendering side of the
//! courted `scx/window-init` + `scx/apply` + `scx/disable` decisions.
//!
//! The DECISIONS live in the scx crate (`window.rs`, `apply.rs` — courted);
//! this model is the UI's projection of them: the combo items, the running
//! label, the profile-visibility rule, the flags text, and the enable/disable
//! of the management widgets.

use cachyos_kernel_manager_scx::config::SchedMode;
use cachyos_kernel_manager_scx::window::{
    apply_ui_decision, flags_text, profile_ui_visible, InitStep,
};

/// The UI state of the sched-ext window.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // fields documented below; the window semantics are courted by scx/*
pub struct ScxWindowModel {
    /// The scheduler combo items (the loader's supported schedulers).
    pub schedulers: Vec<String>,
    /// The selected scheduler (combo selection).
    pub scheduler: String,
    /// The running-scheduler label.
    pub running_scheduler: String,
    /// Whether the scheduler-management widgets are enabled (the loader is
    /// reachable).
    pub enabled: bool,
    /// Whether the profile row is visible (scx_bpfland/scx_lavd only).
    pub profile_visible: bool,
    /// The selected profile label ("Auto".."Server").
    pub profile: String,
    /// The flags text input value.
    pub flags: String,
    /// The critical dialog to show at init (config/loader failure).
    pub critical: Option<String>,
    /// The config path (for the apply/disable mutations).
    pub config_path: String,
}

impl ScxWindowModel {
    /// Build from the courted init trace (`scx::window::window_init`).
    pub fn from_init_steps(
        steps: &[InitStep],
        config_path: String,
        default_profile: &str,
    ) -> ScxWindowModel {
        let mut model = ScxWindowModel {
            schedulers: Vec::new(),
            scheduler: String::new(),
            running_scheduler: String::new(),
            enabled: false,
            profile_visible: false,
            profile: default_profile.to_string(),
            flags: String::new(),
            critical: None,
            config_path,
        };
        for step in steps {
            match step {
                InitStep::CriticalConfigInit => {
                    model.critical = Some("Cannot initialize scx_loader configuration".into());
                    return model;
                }
                InitStep::CriticalNoLoader => {
                    model.critical = Some(
                        "Cannot get information from scx_loader!\nIs it working?\nThis is needed for the app to work properly".into(),
                    );
                    model.enabled = false;
                    return model;
                }
                InitStep::SchedulerCombo { items } => model.schedulers = items.clone(),
                InitStep::InitialScheduler { scheduler } => {
                    if let Some(s) = scheduler {
                        model.scheduler = s.clone();
                    }
                }
                InitStep::ProfileCombo => {}
                InitStep::InitialProfile { mode } => {
                    if let Some(mode) = mode {
                        model.profile = mode.label().to_string();
                    }
                }
                InitStep::CurrentSchedulerLabel { label } => {
                    model.running_scheduler = label.clone();
                }
                InitStep::ProfileVisibility { visible } => model.profile_visible = *visible,
                InitStep::InitialFlags { text } => {
                    model.flags = text.clone();
                    model.enabled = true;
                }
            }
        }
        model
    }

    /// The `on_sched_changed` handler (`schedext-window-internal.cpp:250-264`):
    /// the profile row visibility follows the scheduler; the flags re-render
    /// for the current profile.
    pub fn on_sched_changed(
        &mut self,
        scheduler: &str,
        config: &cachyos_kernel_manager_scx::config::SchedConfig,
    ) {
        self.scheduler = scheduler.to_string();
        self.profile_visible = profile_ui_visible(scheduler);
        self.flags = flags_text(config, scheduler, &self.profile);
    }

    /// The `on_sched_profile_changed` handler: the flags re-render for the
    /// selected profile.
    pub fn on_profile_changed(
        &mut self,
        profile: &str,
        config: &cachyos_kernel_manager_scx::config::SchedConfig,
    ) {
        self.profile = profile.to_string();
        self.flags = flags_text(config, &self.scheduler, profile);
    }

    /// The apply button: the courted `ApplyUiDecision` for the current UI
    /// state (the dialog text on failure).
    pub fn apply_decision(&self, apply_succeeded: bool) -> Option<String> {
        apply_ui_decision(&self.scheduler, &self.profile, &self.flags, apply_succeeded).critical
    }

    /// The disable button: the courted failure dialog.
    pub fn disable_critical(&self, disable_succeeded: bool) -> Option<String> {
        cachyos_kernel_manager_scx::window::disable_ui_decision(disable_succeeded)
    }

    /// The apply call's mode (from the profile label — `SchedMode`).
    pub fn scx_mode(&self) -> SchedMode {
        cachyos_kernel_manager_scx::config::mode_from_label(&self.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_kernel_manager_scx::config::default_config;
    use cachyos_kernel_manager_scx::window::{window_init, WindowInitInput};

    fn model_ok() -> ScxWindowModel {
        let mut config = default_config();
        config.default_sched = Some(cachyos_kernel_manager_scx::config::SupportedSched::Bpfland);
        config.default_mode = Some(SchedMode::Gaming);
        let steps = window_init(&WindowInitInput {
            config_init_failed: false,
            supported_scheds: Ok(vec!["scx_bpfland".into(), "scx_lavd".into()]),
            config,
            current_scheduler_label: "scx_bpfland".into(),
        });
        ScxWindowModel::from_init_steps(&steps, "/etc/scx/config.toml".into(), "Auto")
    }

    #[test]
    fn init_populates_ui_state_from_courted_trace() {
        let m = model_ok();
        assert_eq!(m.schedulers, vec!["scx_bpfland", "scx_lavd"]);
        assert_eq!(m.scheduler, "scx_bpfland");
        assert_eq!(m.running_scheduler, "scx_bpfland");
        assert!(m.enabled);
        assert!(m.profile_visible); // bpfland
        assert_eq!(m.profile, "Gaming");
        assert_eq!(m.flags, "-m performance");
        assert_eq!(m.critical, None);
    }

    #[test]
    fn init_without_loader_disables_widgets() {
        let steps = window_init(&WindowInitInput {
            config_init_failed: false,
            supported_scheds: Err("no loader".into()),
            config: default_config(),
            current_scheduler_label: "unknown".into(),
        });
        let m = ScxWindowModel::from_init_steps(&steps, "/etc/scx/config.toml".into(), "Auto");
        assert!(!m.enabled);
        assert_eq!(
            m.critical.as_deref(),
            Some("Cannot get information from scx_loader!\nIs it working?\nThis is needed for the app to work properly")
        );
    }

    #[test]
    fn sched_change_rerenders_flags_and_visibility() {
        let mut m = model_ok();
        let config = default_config();
        m.on_sched_changed("scx_lavd", &config);
        assert!(m.profile_visible);
        m.on_sched_changed("scx_rusty", &config);
        assert!(!m.profile_visible);
        assert_eq!(m.flags, "");
    }

    #[test]
    fn apply_and_disable_decisions_surface_dialogs() {
        let m = model_ok();
        assert_eq!(m.apply_decision(true), None);
        assert_eq!(
            m.apply_decision(false).as_deref(),
            Some("Cannot set default scx scheduler with mode! Scheduler scx_bpfland with mode Gaming")
        );
        assert_eq!(m.disable_critical(true), None);
        assert_eq!(
            m.disable_critical(false).as_deref(),
            Some("Cannot disable scx_loader")
        );
    }
}
