//! Reference harness reproducing the ORACLE's sched-ext surface byte-for-byte
//! (the pre-extraction `scx-manager` at upstream commit `f3eeaf6` +
//! `scx_loader 1.0.9`, archived in `oracle/scx-authority/`, revision
//! `6b4a373e`):
//!
//! - button visibility (`km-window.cpp:185-188`): the sched-ext button is
//!   hidden unless `/sys/kernel/sched_ext/state` exists;
//! - `get_current_scheduler` (`schedext-window-internal.cpp:57-72`): state
//!   != "enabled" → the state text; enabled + empty ops → "unknown"; else
//!   the ops text;
//! - the default per-mode flags matrix + config-override/fallback
//!   (`scx_loader/src/config.rs`);
//! - the SchedExtWindow init sequence (`schedext-window-internal.cpp:120-190`);
//! - the profile visibility + flags render (`on_sched_changed` /
//!   `on_sched_profile_changed`, 250-264 / 199-214);
//! - the apply trace (`apply_scheduler_change`,
//!   `config-option-lib/src/scx_loader_config.rs`): service disable,
//!   args-vs-mode (b70b01b), loader enable, pkexec copy;
//! - the disable trace (`disable_scheduler`): stop_scheduler + pkexec copy,
//!   default_sched cleared;
//! - the `org.scx.Loader` interface (`scx_loader/src/dbus.rs`).
//!
//! Input: the shared corpus schema (`cachyos-km-scx-corpus-v1`). Output:
//! the surface JSON. This tool is court evidence infrastructure, never
//! shipped.

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

// ---------------------------------------------------------------------------
// Surface 1: button visibility (km-window.cpp:185-188)
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct ButtonCorpus {
    state_file_exists: bool,
}

fn button(c: &ButtonCorpus) -> serde_json::Value {
    json!({
        "schema": "cachyos-km-scx-button-v1",
        "visible": c.state_file_exists,
    })
}

// ---------------------------------------------------------------------------
// Surface 2: current scheduler (schedext-window-internal.cpp:57-72)
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct CurrentSchedCorpus {
    state_contents: String,
    ops_contents: String,
}

fn current_scheduler(state: &str, ops: &str) -> String {
    if state != "enabled" {
        return state.to_string();
    }
    if ops.is_empty() {
        return "unknown".to_string();
    }
    ops.to_string()
}

fn current_sched(c: &CurrentSchedCorpus) -> serde_json::Value {
    json!({
        "schema": "cachyos-km-scx-current-sched-v1",
        "label": current_scheduler(&c.state_contents, &c.ops_contents),
    })
}

// ---------------------------------------------------------------------------
// Surface 3: mode flags (scx_loader/src/config.rs)
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct ModeFlagsCorpus {
    sched: String,
    mode: String,
    #[serde(default)]
    config_toml: Option<String>,
}

fn sched_of(name: &str) -> Option<&'static str> {
    match name {
        "scx_bpfland" => Some("bpfland"),
        "scx_rusty" => Some("rusty"),
        "scx_lavd" => Some("lavd"),
        "scx_flash" => Some("flash"),
        _ => None,
    }
}

fn mode_of(label: &str) -> &'static str {
    match label {
        "Gaming" => "gaming",
        "Lowlatency" => "lowlatency",
        "Powersave" => "powersave",
        "Server" => "server",
        _ => "auto",
    }
}

/// `get_default_scx_flags_for_mode` (scx_loader/src/config.rs:169-189).
fn default_flags(sched: &str, mode: &str) -> Vec<&'static str> {
    match (sched, mode) {
        ("bpfland", "gaming") => vec!["-m", "performance"],
        ("bpfland", "lowlatency") => {
            vec!["-s", "5000", "-S", "500", "-l", "5000", "-m", "performance"]
        }
        ("bpfland", "powersave") => vec!["-m", "powersave"],
        ("bpfland", "server") => vec!["-p"],
        ("bpfland", "auto") => vec![],
        ("lavd", "gaming") | ("lavd", "lowlatency") => vec!["--performance"],
        ("lavd", "powersave") => vec!["--powersave"],
        ("lavd", "server") | ("lavd", "auto") => vec![],
        _ => vec![], // rusty, flash: no modes
    }
}

/// Parse a minimal config TOML for the `scheds.<name>.<mode>_mode` field.
/// Only the field the surface reads matters (the authority's serde shape:
/// `default_sched = "..."`, `default_mode = "..."`, `[scheds.scx_bpfland]`
/// with `auto_mode = [...]` etc.).
fn config_mode_flags(toml_content: Option<&str>, sched: &str, mode: &str) -> Option<Vec<String>> {
    let content = toml_content?;
    let field = format!("{mode}_mode");
    // find the [scheds.<name>] section and the <field> = [...] line
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line == format!("[scheds.{sched}]");
            continue;
        }
        if in_section && line.starts_with(&format!("{field} =")) {
            let rhs = line.split_once('=').map(|(_, r)| r.trim()).unwrap_or("");
            let inner = rhs.trim_start_matches('[').trim_end_matches(']');
            if inner.is_empty() {
                return Some(Vec::new());
            }
            let parts: Vec<String> = inner
                .split(',')
                .filter_map(|p| {
                    let p = p.trim().trim_matches('"').to_string();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p)
                    }
                })
                .collect();
            return Some(parts);
        }
    }
    None
}

fn mode_flags(c: &ModeFlagsCorpus) -> serde_json::Value {
    let sched = sched_of(&c.sched).unwrap_or("bpfland");
    let mode = mode_of(&c.mode);
    let flags = match config_mode_flags(c.config_toml.as_deref(), &c.sched, mode) {
        Some(f) => f,
        None => default_flags(sched, mode)
            .into_iter()
            .map(String::from)
            .collect(),
    };
    json!({ "schema": "cachyos-km-scx-flags-v1", "flags": flags })
}

// ---------------------------------------------------------------------------
// Surface 4: window init (schedext-window-internal.cpp:120-190)
// ---------------------------------------------------------------------------
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

/// The default config (`scx_loader/src/config.rs:75-98`): default_sched
/// None, default_mode Auto — enough for the init surface.
fn default_defaults() -> (Option<String>, u8) {
    (None, 0)
}

fn config_defaults(toml_content: Option<&str>) -> (Option<String>, u8) {
    let Some(content) = toml_content else {
        return default_defaults();
    };
    let mut default_sched = None;
    let mut default_mode = 0u8;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("default_sched =") {
            default_sched = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = line.strip_prefix("default_mode =") {
            default_mode = match v.trim().trim_matches('"') {
                "Gaming" => 1,
                "PowerSave" | "Powersave" => 2,
                "LowLatency" | "Lowlatency" => 3,
                "Server" => 4,
                _ => 0,
            };
        }
    }
    (default_sched, default_mode)
}

fn profile_visible(sched: &str) -> bool {
    sched == "scx_bpfland" || sched == "scx_lavd"
}

fn window_init(c: &WindowInitCorpus) -> serde_json::Value {
    let mut steps: Vec<serde_json::Value> = Vec::new();
    if c.config_init_failed {
        steps.push(json!({ "kind": "critical-config-init" }));
        return json!({ "schema": "cachyos-km-scx-window-init-v1", "steps": steps });
    }
    if c.supported_scheds_ok {
        steps.push(json!({ "kind": "scheduler-combo", "items": c.supported_scheds }));
        let (default_sched, default_mode) = config_defaults(c.config_toml.as_deref());
        steps.push(json!({ "kind": "initial-scheduler", "scheduler": default_sched }));
        steps.push(json!({ "kind": "profile-combo" }));
        steps.push(json!({ "kind": "initial-profile", "mode": default_mode }));
        steps.push(json!({
            "kind": "current-scheduler-label",
            "label": c.current_scheduler_label,
        }));
        let initial = default_sched.unwrap_or_default();
        steps.push(json!({ "kind": "profile-visibility", "visible": profile_visible(&initial) }));
        // the initial flags: the config entry's mode field, else defaults
        let mode = match default_mode {
            1 => "gaming",
            2 => "powersave",
            3 => "lowlatency",
            4 => "server",
            _ => "auto",
        };
        let flags = match config_mode_flags(c.config_toml.as_deref(), &initial, mode) {
            Some(f) => f,
            None => {
                let s = sched_of(&initial).unwrap_or("bpfland");
                default_flags(s, mode)
                    .into_iter()
                    .map(String::from)
                    .collect()
            }
        };
        steps.push(json!({ "kind": "initial-flags", "text": flags.join(" ") }));
    } else {
        steps.push(json!({ "kind": "critical-no-loader" }));
    }
    json!({ "schema": "cachyos-km-scx-window-init-v1", "steps": steps })
}

// ---------------------------------------------------------------------------
// Surface 5: profile visibility + flags (on_sched_changed / _profile_changed)
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct ProfileCorpus {
    scheduler: String,
    profile_label: String,
    #[serde(default)]
    config_toml: Option<String>,
}

fn profile(c: &ProfileCorpus) -> serde_json::Value {
    let mode = mode_of(&c.profile_label);
    let flags = match config_mode_flags(c.config_toml.as_deref(), &c.scheduler, mode) {
        Some(f) => f,
        None => {
            let s = sched_of(&c.scheduler).unwrap_or("bpfland");
            default_flags(s, mode)
                .into_iter()
                .map(String::from)
                .collect()
        }
    };
    json!({
        "schema": "cachyos-km-scx-profile-v1",
        "profile_ui_visible": profile_visible(&c.scheduler),
        "flags_text": flags.join(" "),
    })
}

// ---------------------------------------------------------------------------
// Surface 6: apply trace (scx_loader_config.rs apply_scheduler_change)
// ---------------------------------------------------------------------------
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

fn apply(c: &ApplyCorpus) -> serde_json::Value {
    let mut steps: Vec<serde_json::Value> = Vec::new();
    // 1. disable_scx_service
    if c.scx_service_enabled {
        steps.push(json!({ "kind": "disable-scx-service" }));
        steps.push(json!({ "kind": "stdout", "line": "Disabling scx service" }));
    } else if c.scx_service_active {
        steps.push(json!({ "kind": "stop-scx-service" }));
        // NOTE: the oracle's typo is the contract
        steps.push(json!({ "kind": "stdout", "line": "Stoping scx service" }));
    }
    // 2. args-vs-mode (b70b01b)
    let mode = mode_of(&c.scx_mode);
    let default_args = match config_mode_flags(c.config_toml.as_deref(), &c.scx_name, mode) {
        Some(f) => f,
        None => {
            let s = sched_of(&c.scx_name).unwrap_or("bpfland");
            default_flags(s, mode)
                .into_iter()
                .map(String::from)
                .collect()
        }
    };
    let mut sched_args: Vec<String> = Vec::new();
    if !c.extra_flags.is_empty() {
        sched_args.extend(c.extra_flags.split(' ').map(String::from));
    }
    if sched_args == default_args {
        steps.push(json!({
            "kind": "stdout",
            "line": format!("Applying scx '{}' with mode {}", c.scx_name, mode_label(c.scx_mode.as_str())),
        }));
        steps.push(json!({
            "kind": "db",
            "call": format!("switch_scheduler({}, {})", c.scx_name, mode_label(c.scx_mode.as_str())),
        }));
        if !c.db_ok {
            steps.push(json!({
                "kind": "stdout",
                "line": format!("Failed to switch '{}' with mode {}: {}", c.scx_name, mode_label(c.scx_mode.as_str()), c.db_error),
            }));
        }
    } else {
        steps.push(json!({
            "kind": "stdout",
            "line": format!("Applying scx '{}' with args: {}", c.scx_name, sched_args.join(" ")),
        }));
        steps.push(json!({
            "kind": "db",
            "call": format!("switch_scheduler_with_args({}, {:?})", c.scx_name, sched_args),
        }));
        if !c.db_ok {
            steps.push(json!({
                "kind": "stdout",
                "line": format!("Failed to switch '{}' with args: {:?}: {}", c.scx_name, sched_args, c.db_error),
            }));
        }
    }
    // 3. enable the loader service
    if !c.scx_loader_service_enabled {
        steps.push(json!({ "kind": "stdout", "line": "Enabling scx_loader service" }));
        steps.push(json!({ "kind": "enable-scx-loader-service" }));
    }
    // 4. persist + pkexec copy
    steps.push(json!({ "kind": "pkexec-copy", "config_path": c.config_path }));
    json!({ "schema": "cachyos-km-scx-apply-v1", "steps": steps })
}

/// The SchedMode Debug rendering used in the oracle's println.
fn mode_label(label: &str) -> &'static str {
    match label {
        "Gaming" => "Gaming",
        "PowerSave" | "Powersave" => "PowerSave",
        "LowLatency" | "Lowlatency" => "LowLatency",
        "Server" => "Server",
        _ => "Auto",
    }
}

// ---------------------------------------------------------------------------
// Surface 7: disable trace (scx_loader_config.rs disable_scheduler)
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
struct DisableCorpus {
    #[serde(default)]
    config_path: String,
    #[serde(default)]
    config_toml: Option<String>,
}

fn disable(c: &DisableCorpus) -> serde_json::Value {
    let steps = vec![
        json!({ "kind": "db", "call": "stop_scheduler()" }),
        json!({ "kind": "pkexec-copy", "config_path": c.config_path }),
    ];
    let default_sched_before = c
        .config_toml
        .as_deref()
        .and_then(|t| config_defaults(Some(t)).0);
    json!({
        "schema": "cachyos-km-scx-disable-v1",
        "steps": steps,
        "default_sched_before": default_sched_before,
        "default_sched_after": serde_json::Value::Null,
    })
}

// ---------------------------------------------------------------------------
// Surface 8: the org.scx.Loader interface (scx_loader/src/dbus.rs)
// ---------------------------------------------------------------------------
fn interface() -> serde_json::Value {
    // D-Bus method/property NAMES are the PascalCase forms zbus derives
    // from the snake_case Rust names (zbus_macros 5.5.0 utils.rs::pascal_case).
    let methods = [
        (
            "StartScheduler",
            vec![("scx_name", "s"), ("sched_mode", "u")],
        ),
        (
            "StartSchedulerWithArgs",
            vec![("scx_name", "s"), ("scx_args", "as")],
        ),
        ("StopScheduler", vec![]),
        (
            "SwitchScheduler",
            vec![("scx_name", "s"), ("sched_mode", "u")],
        ),
        (
            "SwitchSchedulerWithArgs",
            vec![("scx_name", "s"), ("scx_args", "as")],
        ),
    ];
    let methods_json: Vec<serde_json::Value> = methods
        .iter()
        .map(|(name, args)| {
            json!({
                "name": name,
                "in_args": args.iter().map(|(n, t)| json!({"name": n, "type": t})).collect::<Vec<_>>(),
                "out_args": Vec::<String>::new(),
            })
        })
        .collect();
    let properties = [
        ("CurrentScheduler", "s", "read"),
        ("SchedulerMode", "u", "read"),
        ("SupportedSchedulers", "as", "read"),
    ];
    let properties_json: Vec<serde_json::Value> = properties
        .iter()
        .map(|(name, t, access)| json!({"name": name, "type": t, "access": access}))
        .collect();
    json!({
        "schema": "cachyos-km-scx-interface-v1",
        "interface": "org.scx.Loader",
        "service": "org.scx.Loader",
        "path": "/org/scx/Loader",
        "methods": methods_json,
        "properties": properties_json,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [surface, cmd, path] = args.as_slice() else {
        eprintln!(
            "usage: scx-oracle-ref <surface> parse <corpus.json> (or: scx-oracle-ref interface)"
        );
        return ExitCode::from(2);
    };
    let payload = if surface == "interface" && cmd == "parse" {
        // the interface is fixed; the corpus argument is accepted for a
        // uniform runner
        let _ = path;
        interface()
    } else if cmd != "parse" {
        eprintln!("usage: scx-oracle-ref <surface> parse <corpus.json>");
        return ExitCode::from(2);
    } else {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        match surface.as_str() {
            "button-visibility" => {
                let c: ButtonCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                button(&c)
            }
            "current-scheduler" => {
                let c: CurrentSchedCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                current_sched(&c)
            }
            "mode-flags" => {
                let c: ModeFlagsCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                mode_flags(&c)
            }
            "window-init" => {
                let c: WindowInitCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                window_init(&c)
            }
            "profile" => {
                let c: ProfileCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                profile(&c)
            }
            "apply" => {
                let c: ApplyCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                apply(&c)
            }
            "disable" => {
                let c: DisableCorpus = match serde_json::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };
                disable(&c)
            }
            other => {
                eprintln!("unknown surface: {other:?}");
                return ExitCode::from(2);
            }
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
