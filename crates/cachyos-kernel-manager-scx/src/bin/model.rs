//! `cachyos-kernel-manager-scx-model` — candidate witness for the `scx/*`
//! courts. Reads the SAME corpus schema as `tools/scx-oracle-ref` and
//! renders the candidate's REAL models (the scx crate): the button
//! visibility, the sysfs current-scheduler readback, the mode flags, the
//! window-init trace, the profile visibility + flags, the apply trace, and
//! the disable trace.
//!
//! Usage: cachyos-kernel-manager-scx-model <surface> parse <corpus.json>
//! surfaces: button | current-scheduler | mode-flags | window-init |
//!           profile | apply | disable

use cachyos_kernel_manager_scx::apply::{
    apply_trace, disable_config_mutation, disable_trace, ApplyInput, DbResult,
};
use cachyos_kernel_manager_scx::config::{
    flags_for_mode, parse_config, SchedConfig, SchedMode, SupportedSched,
};
use cachyos_kernel_manager_scx::state::current_scheduler;
use cachyos_kernel_manager_scx::window::{
    flags_text, main_window_schedext_visible, profile_ui_visible, window_init, InitStep,
    WindowInitInput,
};
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct ButtonCorpus {
    state_file_exists: bool,
}

#[derive(Debug, Deserialize)]
struct CurrentSchedCorpus {
    state_contents: String,
    ops_contents: String,
}

#[derive(Debug, Deserialize)]
struct ModeFlagsCorpus {
    sched: String,
    mode: String,
    #[serde(default)]
    config_toml: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WindowInitCorpus {
    #[serde(default)]
    config_init_failed: bool,
    #[serde(default)]
    supported_scheds_ok: bool,
    #[serde(default)]
    supported_scheds: Vec<String>,
    #[serde(default)]
    config_toml: Option<String>,
    #[serde(default)]
    current_scheduler_label: String,
}

#[derive(Debug, Deserialize)]
struct ProfileCorpus {
    scheduler: String,
    profile_label: String,
    #[serde(default)]
    config_toml: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApplyCorpus {
    scx_name: String,
    scx_mode: String,
    #[serde(default)]
    extra_flags: String,
    #[serde(default)]
    config_toml: Option<String>,
    #[serde(default)]
    scx_service_enabled: bool,
    #[serde(default)]
    scx_service_active: bool,
    #[serde(default)]
    scx_loader_service_enabled: bool,
    #[serde(default)]
    config_path: String,
    #[serde(default)]
    db_ok: bool,
    #[serde(default)]
    db_error: String,
}

#[derive(Debug, Deserialize)]
struct DisableCorpus {
    #[serde(default)]
    config_path: String,
    #[serde(default)]
    config_toml: Option<String>,
}

fn parse_corpus(path: &str) -> Result<serde_json::Value, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn load_config(toml_content: Option<&str>) -> Result<SchedConfig, String> {
    match toml_content {
        Some(content) => parse_config(content),
        None => Ok(cachyos_kernel_manager_scx::config::default_config()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [surface, cmd, path] = args.as_slice() else {
        eprintln!("usage: cachyos-kernel-manager-scx-model <surface> parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: cachyos-kernel-manager-scx-model <surface> parse <corpus.json>");
        return ExitCode::from(2);
    }
    let value = match parse_corpus(path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let payload = match render(surface, value) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}

fn render(surface: &str, value: serde_json::Value) -> Result<serde_json::Value, String> {
    match surface {
        "button-visibility" => {
            let c: ButtonCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            Ok(
                json!({ "schema": "cachyos-km-scx-button-v1", "visible": main_window_schedext_visible(c.state_file_exists) }),
            )
        }
        "current-scheduler" => {
            let c: CurrentSchedCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            Ok(
                json!({ "schema": "cachyos-km-scx-current-sched-v1", "label": current_scheduler(&c.state_contents, &c.ops_contents) }),
            )
        }
        "mode-flags" => {
            let c: ModeFlagsCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let sched = parse_sched(&c.sched);
            let mode = parse_mode_label(&c.mode);
            let config = load_config(c.config_toml.as_deref())?;
            Ok(
                json!({ "schema": "cachyos-km-scx-flags-v1", "flags": flags_for_mode(&config, sched, mode) }),
            )
        }
        "window-init" => {
            let c: WindowInitCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let config = load_config(c.config_toml.as_deref())?;
            let supported = if c.supported_scheds_ok {
                Ok(c.supported_scheds.clone())
            } else {
                Err("no loader".to_string())
            };
            let steps = window_init(&WindowInitInput {
                config_init_failed: c.config_init_failed,
                supported_scheds: supported,
                config,
                current_scheduler_label: c.current_scheduler_label.clone(),
            });
            Ok(json!({
                "schema": "cachyos-km-scx-window-init-v1",
                "steps": steps.iter().map(init_step_json).collect::<Vec<_>>(),
            }))
        }
        "profile" => {
            let c: ProfileCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let config = load_config(c.config_toml.as_deref())?;
            Ok(json!({
                "schema": "cachyos-km-scx-profile-v1",
                "profile_ui_visible": profile_ui_visible(&c.scheduler),
                "flags_text": flags_text(&config, &c.scheduler, &c.profile_label),
            }))
        }
        "apply" => {
            let c: ApplyCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let config = load_config(c.config_toml.as_deref())?;
            let input = ApplyInput {
                scx_name: c.scx_name,
                scx_mode: parse_mode_label(&c.scx_mode),
                extra_flags: c.extra_flags,
                config,
                scx_service_enabled: c.scx_service_enabled,
                scx_service_active: c.scx_service_active,
                scx_loader_service_enabled: c.scx_loader_service_enabled,
                config_path: c.config_path,
                db_result: if c.db_ok {
                    DbResult::Ok
                } else {
                    DbResult::Fail
                },
                db_error: c.db_error,
            };
            Ok(json!({ "schema": "cachyos-km-scx-apply-v1", "steps": apply_trace(&input) }))
        }
        "disable" => {
            let c: DisableCorpus = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let config = load_config(c.config_toml.as_deref())?;
            let mutated = disable_config_mutation(&config);
            Ok(json!({
                "schema": "cachyos-km-scx-disable-v1",
                "steps": disable_trace(&c.config_path),
                "default_sched_before": config.default_sched.map(|s| s.name()),
                "default_sched_after": mutated.default_sched.map(|s| s.name()),
            }))
        }
        other => Err(format!("unknown surface: {other:?}")),
    }
}

fn parse_sched(name: &str) -> SupportedSched {
    name.parse().unwrap_or(SupportedSched::Bpfland)
}

fn parse_mode_label(label: &str) -> SchedMode {
    cachyos_kernel_manager_scx::config::mode_from_label(label)
}

fn init_step_json(step: &InitStep) -> serde_json::Value {
    match step {
        InitStep::CriticalConfigInit => json!({ "kind": "critical-config-init" }),
        InitStep::SchedulerCombo { items } => json!({ "kind": "scheduler-combo", "items": items }),
        InitStep::CriticalNoLoader => json!({ "kind": "critical-no-loader" }),
        InitStep::InitialScheduler { scheduler } => {
            json!({ "kind": "initial-scheduler", "scheduler": scheduler })
        }
        InitStep::ProfileCombo => json!({ "kind": "profile-combo" }),
        InitStep::InitialProfile { mode } => {
            json!({ "kind": "initial-profile", "mode": mode.map(|m| m.as_u8()) })
        }
        InitStep::CurrentSchedulerLabel { label } => {
            json!({ "kind": "current-scheduler-label", "label": label })
        }
        InitStep::ProfileVisibility { visible } => {
            json!({ "kind": "profile-visibility", "visible": visible })
        }
        InitStep::InitialFlags { text } => json!({ "kind": "initial-flags", "text": text }),
    }
}
