//! The apply/disable decision traces — `apply_scheduler_change` and
//! `disable_scheduler` (`config-option-lib/src/scx_loader_config.rs` at the
//! pre-extraction commit `f3eeaf6`).
//!
//! The traces are deterministic over (config, service states, D-Bus
//! outcome): the service-disable branch, the args-vs-mode decision, the
//! oracle's stdout lines (including the `"Stoping scx service"` typo and
//! the Rust `Debug` renderings), the `systemctl enable -f scx_loader`
//! step, the config mutation, the `/tmp/scx_loader.toml` write, and the
//! `pkexec /usr/bin/cp` copy.

use crate::config::{flags_for_mode, SchedConfig, SchedMode, SupportedSched};
use serde::{Deserialize, Serialize};

/// One step of an apply/disable trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScxStep {
    /// `systemctl disable --now -f scx` (when `scx` is enabled).
    DisableScxService,
    /// `systemctl stop -f scx` (when `scx` is active but not enabled).
    StopScxService,
    /// The oracle's stdout line (`println!`).
    Stdout { line: String },
    /// `systemctl enable -f scx_loader` (when not already enabled).
    EnableScxLoaderService,
    /// `pkexec /usr/bin/cp /tmp/scx_loader.toml <config_path>`.
    PkexecCopy { config_path: String },
    /// The D-Bus call the oracle makes.
    Db { call: String },
}

/// The D-Bus outcome of a switch call (a court input: the error text is
/// implementation-specific, so the corpus fixes the failure text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbResult {
    Ok,
    Fail,
}

/// The inputs the apply decision branches on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyInput {
    pub scx_name: String,
    pub scx_mode: SchedMode,
    pub extra_flags: String,
    pub config: SchedConfig,
    pub scx_service_enabled: bool,
    pub scx_service_active: bool,
    pub scx_loader_service_enabled: bool,
    pub config_path: String,
    pub db_result: DbResult,
    /// The fixed error text used when `db_result == Fail` (the corpus
    /// supplies it; the oracle's real error string is anyhow's Debug).
    pub db_error: String,
}

/// `apply_scheduler_change` (`scx_loader_config.rs`) as a pure decision
/// trace. Byte-exact reconstruction of the ordering, the branches, and the
/// stdout lines. The DECISION lives in [`apply_plan`]; this renders the
/// witness steps from the plan (one decision source, audit P0).
pub fn apply_trace(input: &ApplyInput) -> Vec<ScxStep> {
    let plan = apply_plan(input);
    let mut steps: Vec<ScxStep> = Vec::new();

    // 1. stop/disable 'scx.service' if running/enabled (it would conflict)
    match plan.scx_service_op {
        Some(ServiceOp::DisableScxService) => {
            steps.push(ScxStep::DisableScxService);
            steps.push(ScxStep::Stdout {
                line: "Disabling scx service".to_string(),
            });
        }
        Some(ServiceOp::StopScxService) => {
            steps.push(ScxStep::StopScxService);
            // NOTE: the oracle's typo is part of the contract
            steps.push(ScxStep::Stdout {
                line: "Stoping scx service".to_string(),
            });
        }
        Some(ServiceOp::EnableScxLoaderService) | None => {}
    }

    // 2. the args-vs-mode decision (already made by the plan)
    match &plan.db_call {
        DbCall::SwitchScheduler { sched, mode } => {
            steps.push(ScxStep::Stdout {
                line: format!("Applying scx '{}' with mode {}", sched.name(), mode.debug()),
            });
            steps.push(ScxStep::Db {
                call: format!("switch_scheduler({}, {})", sched.name(), mode.debug()),
            });
            if input.db_result == DbResult::Fail {
                steps.push(ScxStep::Stdout {
                    line: format!(
                        "Failed to switch '{}' with mode {}: {}",
                        input.scx_name,
                        mode.debug(),
                        input.db_error
                    ),
                });
            }
        }
        DbCall::SwitchSchedulerWithArgs { sched, args } => {
            steps.push(ScxStep::Stdout {
                line: format!(
                    "Applying scx '{}' with args: {}",
                    sched.name(),
                    args.join(" ")
                ),
            });
            steps.push(ScxStep::Db {
                call: format!("switch_scheduler_with_args({}, {:?})", sched.name(), args),
            });
            if input.db_result == DbResult::Fail {
                steps.push(ScxStep::Stdout {
                    line: format!(
                        "Failed to switch '{}' with args: {:?}: {}",
                        input.scx_name, args, input.db_error
                    ),
                });
            }
        }
        DbCall::StopScheduler => {}
    }

    // 3. enable the loader service if not enabled (it fully replaces scx)
    if plan.enable_loader {
        steps.push(ScxStep::Stdout {
            line: "Enabling scx_loader service".to_string(),
        });
        steps.push(ScxStep::EnableScxLoaderService);
    }

    // 4. persist: write the temp config, pkexec-copy it
    steps.push(ScxStep::PkexecCopy {
        config_path: plan.config_path.clone(),
    });
    steps
}

/// `disable_scheduler` + `disable_scx_sched` (`scx_loader_config.rs`) as a
/// pure decision trace: clear `default_sched`, write the temp config,
/// `stop_scheduler` D-Bus call, pkexec-copy.
pub fn disable_trace(config_path: &str) -> Vec<ScxStep> {
    vec![
        ScxStep::Db {
            call: "stop_scheduler()".to_string(),
        },
        ScxStep::PkexecCopy {
            config_path: config_path.to_string(),
        },
    ]
}

/// The config mutation `apply_scheduler_change` performs: set
/// `default_sched` + `default_mode` (`set_scx_sched_with_mode`).
pub fn apply_config_mutation(
    config: &SchedConfig,
    scx_name: &str,
    scx_mode: SchedMode,
) -> SchedConfig {
    let mut config = config.clone();
    config.default_sched = scx_name.parse().ok();
    config.default_mode = Some(scx_mode);
    config
}

/// The config mutation `disable_scx_sched` performs: `default_sched = None`.
pub fn disable_config_mutation(config: &SchedConfig) -> SchedConfig {
    let mut config = config.clone();
    config.default_sched = None;
    config
}

// ---------------------------------------------------------------------------
// The structured execution plan (audit P0): the SAME decision tree as the
// witness traces above, but structured so the executor can interpret it
// directly — app.rs must NEVER re-implement this decision tree, and the
// executor must NEVER parse the witness's string-rendered D-Bus calls
// (the reverse-scan antipattern).
// ---------------------------------------------------------------------------

/// A systemd service operation the apply/disable plan performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceOp {
    /// `systemctl disable --now -f scx`.
    DisableScxService,
    /// `systemctl stop -f scx`.
    StopScxService,
    /// `systemctl enable -f scx_loader`.
    EnableScxLoaderService,
}

/// The structured D-Bus call the plan performs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbCall {
    /// `switch_scheduler(sched, mode)` (the args match the mode's defaults).
    SwitchScheduler {
        sched: SupportedSched,
        mode: SchedMode,
    },
    /// `switch_scheduler_with_args(sched, args)` (the args differ from the
    /// mode's defaults).
    SwitchSchedulerWithArgs {
        sched: SupportedSched,
        args: Vec<String>,
    },
    /// `stop_scheduler()`.
    StopScheduler,
}

/// The apply execution plan: the exact operations, in the oracle's order
/// (scx.service conflict -> D-Bus switch -> loader enable -> config
/// persist), with the MUTATED config to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPlan {
    /// The scx.service conflict op (`None` when neither enabled nor active).
    pub scx_service_op: Option<ServiceOp>,
    /// The structured D-Bus call.
    pub db_call: DbCall,
    /// `systemctl enable -f scx_loader` when not already enabled.
    pub enable_loader: bool,
    /// The MUTATED config (default_sched/default_mode set for apply;
    /// default_sched cleared for disable) — the executor persists this.
    pub config: SchedConfig,
    /// The target config path (the pkexec copy destination).
    pub config_path: String,
}

/// The apply decision as a structured plan (the same branches as
/// [`apply_trace`]: the scx.service conflict, the args-vs-mode decision,
/// the loader enable, the config mutation).
pub fn apply_plan(input: &ApplyInput) -> ApplyPlan {
    let scx_service_op = if input.scx_service_enabled {
        Some(ServiceOp::DisableScxService)
    } else if input.scx_service_active {
        Some(ServiceOp::StopScxService)
    } else {
        None
    };
    // the args-vs-mode decision (commit b70b01b): args only when they
    // differ from the mode's defaults
    let sched: SupportedSched = input.scx_name.parse().expect("corpus-validated name");
    let default_args = flags_for_mode(&input.config, sched, input.scx_mode);
    let mut sched_args: Vec<String> = Vec::new();
    if !input.extra_flags.is_empty() {
        sched_args.extend(input.extra_flags.split(' ').map(String::from));
    }
    let db_call = if sched_args == default_args {
        DbCall::SwitchScheduler {
            sched,
            mode: input.scx_mode,
        }
    } else {
        DbCall::SwitchSchedulerWithArgs {
            sched,
            args: sched_args,
        }
    };
    ApplyPlan {
        scx_service_op,
        db_call,
        enable_loader: !input.scx_loader_service_enabled,
        config: apply_config_mutation(&input.config, &input.scx_name, input.scx_mode),
        config_path: input.config_path.clone(),
    }
}

/// The disable decision as a structured plan (`disable_scheduler` +
/// `disable_scx_sched`): stop the scheduler, clear `default_sched`, persist.
pub fn disable_plan(config: &SchedConfig, config_path: &str) -> ApplyPlan {
    ApplyPlan {
        scx_service_op: None,
        db_call: DbCall::StopScheduler,
        enable_loader: false,
        config: disable_config_mutation(config),
        config_path: config_path.to_string(),
    }
}

/// Run one systemd service operation via `pkexec systemctl <args>` (the
/// app runs as the invoking user; the oracle's service ops are root
/// actions).
fn run_service_op(op: ServiceOp) {
    let (sub, args): (&str, &[&str]) = match op {
        ServiceOp::DisableScxService => ("disable", &["--now", "-f", "scx"]),
        ServiceOp::StopScxService => ("stop", &["-f", "scx"]),
        ServiceOp::EnableScxLoaderService => ("enable", &["-f", "scx_loader"]),
    };
    let _ = std::process::Command::new("pkexec")
        .arg("systemctl")
        .arg(sub)
        .args(args)
        .status();
}

/// Execute an [`ApplyPlan`] against the real system — the RUNTIME consumes
/// the model's plan; it never re-implements the decision tree and never
/// parses the witness's string-rendered D-Bus calls (audit P0). Runs in the
/// caller's tokio runtime (the D-Bus calls are async). Returns whether the
/// D-Bus switch/stop call succeeded; the service ops + config persist are
/// best-effort, matching the oracle's sequencing.
#[cfg(feature = "dbus")]
pub async fn execute_apply(plan: &ApplyPlan, connection: &zbus::Connection) -> bool {
    use crate::client::LoaderClientProxy;

    // 1. the scx.service conflict (disable/stop before the switch)
    if let Some(op) = plan.scx_service_op {
        run_service_op(op);
    }

    // 2. the structured D-Bus call (switch, NOT start — the oracle's
    //    apply switches; the old runtime called start_scheduler directly)
    let db_ok = match LoaderClientProxy::new(connection).await {
        Ok(loader) => match &plan.db_call {
            DbCall::SwitchScheduler { sched, mode } => {
                loader.switch_scheduler(sched, *mode).await.is_ok()
            }
            DbCall::SwitchSchedulerWithArgs { sched, args } => {
                loader.switch_scheduler_with_args(sched, args).await.is_ok()
            }
            DbCall::StopScheduler => loader.stop_scheduler().await.is_ok(),
        },
        Err(_) => false,
    };

    // 3. enable the loader service (it fully replaces scx)
    if plan.enable_loader {
        run_service_op(ServiceOp::EnableScxLoaderService);
    }

    // 4. persist the mutated config: write the temp file, pkexec-copy it
    if let Ok(toml) = toml::to_string(&plan.config) {
        if std::fs::write("/tmp/scx_loader.toml", toml).is_ok() {
            let _ = std::process::Command::new("pkexec")
                .args(["/usr/bin/cp", "/tmp/scx_loader.toml", &plan.config_path])
                .status();
        }
    }

    db_ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;

    fn apply_input() -> ApplyInput {
        ApplyInput {
            scx_name: "scx_bpfland".into(),
            scx_mode: SchedMode::Gaming,
            extra_flags: String::new(),
            config: default_config(),
            scx_service_enabled: false,
            scx_service_active: false,
            scx_loader_service_enabled: true,
            config_path: "/etc/scx_loader.toml".into(),
            db_result: DbResult::Ok,
            db_error: String::new(),
        }
    }

    fn kinds(steps: &[ScxStep]) -> Vec<&'static str> {
        steps
            .iter()
            .map(|s| match s {
                ScxStep::DisableScxService => "disable",
                ScxStep::StopScxService => "stop",
                ScxStep::Stdout { .. } => "stdout",
                ScxStep::EnableScxLoaderService => "enable-loader",
                ScxStep::PkexecCopy { .. } => "pkexec",
                ScxStep::Db { .. } => "db",
            })
            .collect()
    }

    #[test]
    fn apply_mode_path_when_args_match_defaults() {
        // the flags edit is pre-filled with the mode's defaults, so an
        // unmodified Apply sends extra_flags == default_args -> the MODE
        // D-Bus call (commit b70b01b: args only when they differ).
        let mut input = apply_input();
        input.extra_flags = "-m performance".into(); // bpfland Gaming defaults
        let steps = apply_trace(&input);
        assert_eq!(kinds(&steps), vec!["stdout", "db", "pkexec"]);
        assert!(steps.iter().any(|s| s
            == &ScxStep::Stdout {
                line: "Applying scx 'scx_bpfland' with mode Gaming".into()
            }));
        assert!(steps.iter().any(|s| s
            == &ScxStep::Db {
                call: "switch_scheduler(scx_bpfland, Gaming)".into()
            }));
    }

    #[test]
    fn apply_args_path_without_flags() {
        // empty flags + Gaming -> sched_args [] != defaults -> the ARGS
        // path with an empty list (the oracle still goes there)
        let steps = apply_trace(&apply_input());
        assert!(steps.iter().any(|s| s
            == &ScxStep::Stdout {
                line: "Applying scx 'scx_bpfland' with args: ".into()
            }));
        assert!(steps.iter().any(|s| s
            == &ScxStep::Db {
                call: "switch_scheduler_with_args(scx_bpfland, [])".into()
            }));
    }

    #[test]
    fn apply_auto_mode_without_flags_uses_mode_path() {
        // Auto mode defaults to [] -> empty flags match -> mode path
        let mut input = apply_input();
        input.scx_mode = SchedMode::Auto;
        let steps = apply_trace(&input);
        assert!(steps.iter().any(|s| s
            == &ScxStep::Stdout {
                line: "Applying scx 'scx_bpfland' with mode Auto".into()
            }));
        assert!(steps.iter().any(|s| s
            == &ScxStep::Db {
                call: "switch_scheduler(scx_bpfland, Auto)".into()
            }));
    }

    #[test]
    fn apply_args_path_when_flags_differ() {
        let mut input = apply_input();
        input.extra_flags = "-m powersave".into();
        let steps = apply_trace(&input);
        assert!(steps.iter().any(|s| s
            == &ScxStep::Stdout {
                line: "Applying scx 'scx_bpfland' with args: -m powersave".into()
            }));
        assert!(steps.iter().any(|s| s
            == &ScxStep::Db {
                call: "switch_scheduler_with_args(scx_bpfland, [\"-m\", \"powersave\"])".into()
            }));
    }

    #[test]
    fn apply_disables_conflicting_scx_service() {
        let mut input = apply_input();
        input.scx_service_enabled = true;
        let steps = apply_trace(&input);
        assert_eq!(steps[0], ScxStep::DisableScxService);
        assert!(steps.contains(&ScxStep::Stdout {
            line: "Disabling scx service".into()
        }));

        let mut input = apply_input();
        input.scx_service_active = true;
        let steps = apply_trace(&input);
        assert_eq!(steps[0], ScxStep::StopScxService);
        // the oracle's typo is the contract
        assert!(steps.contains(&ScxStep::Stdout {
            line: "Stoping scx service".into()
        }));
    }

    #[test]
    fn apply_enables_loader_when_not_enabled() {
        let mut input = apply_input();
        input.scx_loader_service_enabled = false;
        let steps = apply_trace(&input);
        assert!(steps.contains(&ScxStep::Stdout {
            line: "Enabling scx_loader service".into()
        }));
        assert!(steps.contains(&ScxStep::EnableScxLoaderService));
    }

    #[test]
    fn apply_failure_prints_switch_error() {
        let mut input = apply_input();
        input.extra_flags = "-m performance".into(); // mode path
        input.db_result = DbResult::Fail;
        input.db_error = "dbus error".into();
        let steps = apply_trace(&input);
        assert!(steps.contains(&ScxStep::Stdout {
            line: "Failed to switch 'scx_bpfland' with mode Gaming: dbus error".into()
        }));
        // even on D-Bus failure the persist steps still run
        assert!(steps
            .iter()
            .any(|s| matches!(s, ScxStep::PkexecCopy { .. })));
    }

    #[test]
    fn apply_and_disable_mutate_the_config() {
        let config = default_config();
        let applied = apply_config_mutation(&config, "scx_lavd", SchedMode::Server);
        assert_eq!(applied.default_sched, Some(SupportedSched::Lavd));
        assert_eq!(applied.default_mode, Some(SchedMode::Server));
        let disabled = disable_config_mutation(&applied);
        assert_eq!(disabled.default_sched, None);
        assert_eq!(disabled.default_mode, Some(SchedMode::Server));
    }

    #[test]
    fn disable_trace_is_stop_then_copy() {
        let steps = disable_trace("/etc/scx_loader.toml");
        assert_eq!(
            steps,
            vec![
                ScxStep::Db {
                    call: "stop_scheduler()".into()
                },
                ScxStep::PkexecCopy {
                    config_path: "/etc/scx_loader.toml".into()
                },
            ]
        );
    }
}
