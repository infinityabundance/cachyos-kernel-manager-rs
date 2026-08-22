//! Command modeling, argv rendering, and the narrow process-execution
//! boundary.
//!
//! The directive's rule: model commands before rendering them; shell
//! construction is business logic and must not leak into domain models.
//! This crate turns [`CommandPlan`] values into concrete argv vectors at the
//! execution boundary, reproducing the oracle's exact chains
//! (`oracle/upstream/src/utils.cpp`, `src/terminal-helper`,
//! `src/conf-window.cpp`).
//!
//! # Bash is the contract
//!
//! `terminal-helper` and `rootshell.sh` are installed Bash scripts, part of
//! the drop-in package contract (directive §5, §18, §33). This crate does
//! NOT reimplement them in Rust: the packaging layer ships byte-identical
//! copies (courted by `privilege/helper-scripts`) and [`run_cmd_terminal`]
//! invokes the installed script with the exact argv the oracle's
//! `runCmdTerminal` produces.
//!
//! Reconstructed at revision `6b4a373e`; courts: `terminal-helper/*`,
//! `privilege/*`, `transaction-plan/*`.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

/// Fixed helper paths (compile-time constants; overridden in tests).
pub mod paths {
    /// `terminal-helper` install location (CMakeLists.txt:159).
    pub const TERMINAL_HELPER: &str = "/usr/lib/cachyos-kernel-manager/terminal-helper";
    /// `rootshell.sh` install location + polkit annotated exec path.
    pub const ROOTSHELL: &str = "/usr/lib/cachyos-kernel-manager/rootshell.sh";
}

/// The `read -p` suffix appended to every terminal command
/// (`utils.cpp:124`, `conf-window.cpp:363`).
pub const PRESS_ENTER_SUFFIX: &str = "; read -p 'Press enter to exit'";

/// Escalation strategy (`runCmdTerminal` vs `run_cmd_async`,
/// `utils.cpp:122-135`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Escalate {
    /// `-s pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh`
    PkexecRootShell,
    /// no `-s`; the command runs as the invoking user.
    None,
}

/// A modeled external command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandPlan {
    /// `pacman -S [--needed] <packages>` (kernel install).
    InstallRepoPackages { packages: Vec<String>, needed: bool },
    /// `pacman -Rsn <packages>` (kernel removal).
    RemovePackages { packages: Vec<String> },
    /// `sudo pacman -U <globs>` (built-artifact install).
    InstallLocalPackages { globs: Vec<String> },
    /// `makepkg -scf --cleanbuild --skipchecksums` (repo kernel build).
    BuildKernelPackage,
    /// `makepkg -sicf --cleanbuild --skipchecksums` (AUR kernel build).
    BuildAurPackage,
    /// `git clone <url> <dir>` / `git checkout --force master` /
    /// `git clean -fd` / `git pull` (refresh lifecycle).
    GitRefresh { url: String, dir: String },
}

/// Render the raw pacman argv for an install plan (no shell involved).
pub fn pacman_install_argv(packages: &[String], needed: bool) -> Vec<String> {
    let mut argv = vec!["pacman".to_string(), "-S".to_string()];
    if needed {
        argv.push("--needed".to_string());
    }
    argv.extend(packages.iter().cloned());
    argv
}

/// Render the raw pacman argv for a removal plan.
pub fn pacman_remove_argv(packages: &[String]) -> Vec<String> {
    let mut argv = vec!["pacman".to_string(), "-Rsn".to_string()];
    argv.extend(packages.iter().cloned());
    argv
}

/// Render the raw makepkg argv for a repo kernel build.
pub fn makepkg_repo_argv() -> Vec<String> {
    vec![
        "makepkg".into(),
        "-scf".into(),
        "--cleanbuild".into(),
        "--skipchecksums".into(),
    ]
}

/// Render the raw makepkg argv for an AUR kernel build
/// (`aur_kernel.cpp:53`).
pub fn makepkg_aur_argv() -> Vec<String> {
    vec![
        "makepkg".into(),
        "-sicf".into(),
        "--cleanbuild".into(),
        "--skipchecksums".into(),
    ]
}

/// Append the oracle's keep-terminal-open suffix
/// (`utils.cpp:124`; `conf-window.cpp:363`).
pub fn with_pause(cmd: &str) -> String {
    format!("{cmd}{PRESS_ENTER_SUFFIX}")
}

/// Render the oracle's `terminal-helper` argv.
///
/// Oracle (`runCmdTerminal`, `utils.cpp:125-133`):
/// ```text
/// terminal-helper -s pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh <cmd>
/// ```
/// `-s` sets the launcher shell to the pkexec chain; the command is passed
/// as a *single argv element* (terminal-helper writes it to a temp file and
/// runs `$LAUNCHER_CMD "$file"`).
pub fn terminal_helper_argv(cmd: &str, escalate: Escalate) -> Vec<String> {
    let mut argv = vec![paths::TERMINAL_HELPER.to_string()];
    match escalate {
        Escalate::PkexecRootShell => {
            argv.push("-s".to_string());
            argv.push(format!("pkexec {}", paths::ROOTSHELL));
        }
        Escalate::None => {}
    }
    argv.push(with_pause(cmd));
    argv
}

/// The oracle's `run_cmd_async` variant (`conf-window.cpp:361-376`): same
/// helper, no `-s`, working directory set by the caller, no pause suffix
/// added here (it is added by `run_cmd_async` itself — identical).
pub fn terminal_helper_async_argv(cmd: &str) -> Vec<String> {
    terminal_helper_argv(cmd, Escalate::None)
}

/// The oracle's artifact-install command (`finished_proc`,
/// `conf-window.cpp:394-396`): `sudo pacman -U <globs joined by ' '>`.
pub fn artifact_install_command(globs: &[String]) -> String {
    format!("sudo pacman -U {}", globs.join(" "))
}

/// The oracle's build-flow decisions (`conf-window.cpp:696-735` on_execute,
/// `378-405` finished_proc, `aur_kernel.cpp:53`), rendered into the concrete
/// command/path/argv set the Configure→Build flow produces.
///
/// Courted byte-for-byte by `build-env/lifecycle` against
/// `tools/buildflow-oracle-ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFlowPlan {
    /// The selected kernel variant.
    pub variant: cachyos_kernel_manager_core::options::KernelVariant,
    /// `get_kernel_name_path(variant)` — the PKGBUILD dir name
    /// (`conf-window.cpp:124-148`).
    pub cpusched_path: String,
    /// `<fs::current_path()>/<cpusched_path>` (`on_execute:730-731`) — the
    /// oracle's mutable-cwd quirk (D-004).
    pub working_path: String,
    /// `makepkg -scf --cleanbuild --skipchecksums && touch .done-status`
    /// (`on_execute:734`; gap-006: the repo build has NO `-i`).
    pub build_command: String,
    /// `terminal-helper <cmd>; read -p 'Press enter to exit'`
    /// (`run_cmd_async:361-376`).
    pub terminal_argv: Vec<String>,
    /// `<working_path>/.done-status` (`finished_proc:384`) — success is
    /// defined by this file's presence, NOT the exit code.
    pub done_status: String,
    /// `makepkg -sicf --cleanbuild --skipchecksums` (`aur_kernel.cpp:53`;
    /// gap-006: the AUR build ADDS `-i`).
    pub aur_build_command: String,
    /// `sudo pacman -U <globs>` (`finished_proc:394-396`).
    pub artifact_install_command: String,
}

impl BuildFlowPlan {
    /// Render the plan for a variant, the oracle's process cwd, and the
    /// artifact globs from the pkgfuncs probe.
    pub fn render(
        variant: cachyos_kernel_manager_core::options::KernelVariant,
        cwd: &str,
        globs: &[String],
    ) -> BuildFlowPlan {
        let cpusched_path = variant.dir_name().to_string();
        let working_path = format!("{cwd}/{cpusched_path}");
        let build_command =
            "makepkg -scf --cleanbuild --skipchecksums && touch .done-status".to_string();
        let terminal_argv = terminal_helper_async_argv(&build_command);
        let done_status = format!("{working_path}/.done-status");
        let aur_build_command = "makepkg -sicf --cleanbuild --skipchecksums".to_string();
        let artifact_install_command = artifact_install_command(globs);
        BuildFlowPlan {
            variant,
            cpusched_path,
            working_path,
            build_command,
            terminal_argv,
            done_status,
            aur_build_command,
            artifact_install_command,
        }
    }
}

/// One async-process completion (`finished_proc`, `conf-window.cpp:378-405`)
/// — the inputs the oracle branches on: the `.done-status` FILE (the success
/// contract — NOT the exit code), the QProcess exit code (only used in the
/// failure message), the user's install-dialog answer, and the artifact
/// globs (for the install command).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishEvent {
    /// `<m_build_conf_path>/.done-status` exists (`fs::exists`).
    pub done_status_exists: bool,
    /// The terminal-helper process exit code.
    pub exit_code: i32,
    /// The QMessageBox "Do you want to install build packages?" answer;
    /// `None` when the question is not asked (failure path).
    pub user_choice: Option<bool>,
    /// The pkg-glob probe result (success+yes path only).
    pub globs: Vec<String>,
}

/// The decisions `finished_proc` produces for ONE completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishOutcome {
    /// stdout lines in order (the oracle's `fmt::print` without stderr).
    pub stdout: Vec<String>,
    /// stderr lines in order.
    pub stderr: Vec<String>,
    /// `sudo pacman -U <globs>` when the user said Yes (the re-entrant
    /// async command; `None` otherwise).
    pub next_command: Option<String>,
    /// The success path removes `.done-status` (so the NEXT completion
    /// — the pacman install — falls into the failure branch: quirk).
    pub removes_done_status: bool,
    /// `m_running` after this completion.
    pub running_after: bool,
}

/// `finished_proc` (`conf-window.cpp:378-405`) as a pure function.
///
/// Byte-exact reconstruction:
/// - `m_running = false` first;
/// - `.done-status` present → remove it, stdout `success`; ask
///   "Do you want to install build packages?"; on Yes → stdout
///   `pressed yes`, `pacman_cmd := <sudo pacman -U <globs>>`, set
///   `m_running = true` and start the install (re-entrant — its OWN
///   completion re-enters `finished_proc`, where the file is gone → the
///   failure branch, so even a SUCCESSFUL install prints
///   `process failed with exit code: 0` to stderr);
/// - file absent → stderr `process failed with exit code: <exit_code>`.
///
/// The success decision keys on the FILE, never the exit code.
pub fn finished_proc(event: &FinishEvent) -> FinishOutcome {
    let mut outcome = FinishOutcome {
        stdout: Vec::new(),
        stderr: Vec::new(),
        next_command: None,
        removes_done_status: false,
        running_after: false,
    };
    if event.done_status_exists {
        outcome.removes_done_status = true;
        outcome.stdout.push("success".to_string());
        if event.user_choice == Some(true) {
            outcome.stdout.push("pressed yes".to_string());
            let cmd = artifact_install_command(&event.globs);
            outcome.stdout.push(format!("pacman_cmd := {cmd}"));
            outcome.next_command = Some(cmd);
            outcome.running_after = true;
        }
    } else {
        outcome.stderr.push(format!(
            "process failed with exit code: {}\n",
            event.exit_code
        ));
    }
    outcome
}

/// A user action in the Configure window (the `OK`/`Cancel` buttons and the
/// window close — `conf-window.cpp:549-550,688-694`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigureAction {
    /// `OK` → `on_execute`.
    Execute,
    /// `Cancel` → `on_cancel` → `close()`.
    Cancel,
    /// The window-manager close → `closeEvent` (accepted unconditionally).
    Close,
}

/// One trace entry of the Configure-window lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigureTraceEvent {
    pub action: ConfigureAction,
    /// `start` (a build began), `ignored` (the m_running guard returned), or
    /// `closed` (the window closed; the in-flight QProcess member is
    /// destroyed → the child is terminated).
    pub outcome: String,
}

/// The Configure-window lifecycle (`on_execute` guard + close semantics)
/// as a stateful trace.
///
/// Byte-exact reconstruction:
/// - `on_execute` (`conf-window.cpp:696-701`): `if (m_running) { return; }`
///   — a second Execute while a build/install runs is a complete NO-OP
///   (no command, no probe, m_running unchanged);
/// - `on_cancel` → `close()` and `closeEvent` (`688-690`) accepts
///   unconditionally (`QWidget::closeEvent`) — the window is destroyed and
///   the `QProcess m_cmd` member destructor terminates the in-flight child;
/// - after a close/cancel the window is gone: further actions are
///   unreachable in the oracle and emit nothing.
pub fn configure_trace(actions: &[ConfigureAction]) -> (Vec<ConfigureTraceEvent>, bool) {
    let mut running = false;
    let mut trace = Vec::new();
    for action in actions {
        match action {
            ConfigureAction::Execute => {
                if running {
                    trace.push(ConfigureTraceEvent {
                        action: *action,
                        outcome: "ignored".to_string(),
                    });
                } else {
                    running = true;
                    trace.push(ConfigureTraceEvent {
                        action: *action,
                        outcome: "start".to_string(),
                    });
                }
            }
            ConfigureAction::Cancel | ConfigureAction::Close => {
                running = false;
                trace.push(ConfigureTraceEvent {
                    action: *action,
                    outcome: "closed".to_string(),
                });
                break; // window destroyed; the oracle cannot receive more
            }
        }
    }
    (trace, running)
}

// ---------------------------------------------------------------------------
// Execution boundary (directive §18: Bash semantics stay in Bash; here we
// reproduce the oracle's popen/QProcess invocation semantics EXACTLY).
// ---------------------------------------------------------------------------

/// `utils::exec` (`utils.cpp:99-120`) — the oracle's `popen(cmd, "r")`
/// wrapper.
///
/// Parity contract (all courted by the Phase 5 probe/transaction courts):
/// - the command runs through `/bin/sh -c -- <cmd>` — glibc's popen on the
///   CachyOS toolchain (glibc ≥ 2.44) execs `sh -c -- <command>` (the `--`
///   guards command strings starting with `-`); on older glibc the `--` is
///   absent — an environment-dependent argv surface recorded in the atlas;
/// - **stdout** is captured; **stderr is inherited** (popen does not capture
///   it);
/// - exactly ONE trailing `\n` is stripped (a double-newline stays
///   `"a\n"` — `result.ends_with('\n')` then one `pop_back`);
/// - the exit status is IGNORED (popen returns whatever was read; the
///   oracle never checks `pclose`);
/// - if popen itself fails: prints `popen failed! '<cmd>'` to stderr and
///   returns `"-1"`.
pub fn exec_shell(command: &str) -> String {
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg("--")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output();
    match out {
        Ok(o) => {
            let mut result = String::from_utf8_lossy(&o.stdout).into_owned();
            if result.ends_with('\n') {
                result.pop();
            }
            result
        }
        Err(e) => {
            eprintln!("popen failed! '{command}'");
            // NOTE: the oracle prints the popen failure but NOT errno text
            // (only the command); keep the parity string exact.
            let _ = e;
            "-1".to_string()
        }
    }
}

/// The oracle's `runCmdTerminal` (`utils.cpp:122-135`) — spawn
/// `terminal-helper` and wait indefinitely.
///
/// Parity contract:
/// - appends `; read -p 'Press enter to exit'` to the command;
/// - `-s pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh` when
///   escalating;
/// - waits FOREVER (`waitForFinished(-1)`);
/// - returns the helper's exit code;
/// - when the helper FAILED TO START, QProcess::exitCode() is `0` (the
///   oracle never checks `FailedToStart` here — gap-008) — reproduced
///   literally below.
pub fn run_cmd_terminal(cmd: &str, escalate: Escalate) -> i32 {
    let argv = terminal_helper_argv(cmd, escalate);
    let mut proc = Command::new(&argv[0]);
    for arg in &argv[1..] {
        proc.arg(arg);
    }
    match proc.status() {
        Ok(status) => status.code().unwrap_or(0),
        Err(_) => 0, // FailedToStart parity: exitCode() == 0
    }
}

/// The oracle's `run_process` (`utils.cpp:137-151`) — QProcess with
/// `ForwardedChannels`, wait, `FailedToStart → -1`, else the exit code.
pub fn run_process(program: &str, args: &[String]) -> i32 {
    let mut proc = Command::new(program);
    proc.args(args);
    proc.stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    match proc.status() {
        // signal death: QProcess::exitCode() returns 0 (CrashExit)
        Ok(status) => status.code().unwrap_or(0),
        Err(_) => -1, // FailedToStart
    }
}

#[cfg(test)]
mod exec_tests {
    use super::*;

    #[test]
    fn exec_shell_captures_stdout_and_strips_one_newline() {
        assert_eq!(exec_shell("printf 'hello\\n'"), "hello");
        // exactly ONE trailing newline is stripped (popen parity)
        assert_eq!(exec_shell("printf 'a\\n\\n'"), "a\n");
        assert_eq!(exec_shell("printf 'no-newline'"), "no-newline");
    }

    #[test]
    fn exec_shell_ignores_exit_status() {
        // a failing command still returns its stdout (popen parity)
        assert_eq!(exec_shell("echo data; exit 3"), "data");
        assert_eq!(exec_shell("exit 1"), "");
    }

    #[test]
    fn exec_shell_runs_through_sh_dash_c() {
        // pipeline + shell builtins work exactly as in popen
        assert_eq!(exec_shell("echo b | cat"), "b");
    }

    #[test]
    fn exec_shell_failure_returns_minus_one_string() {
        // nonexistent binary: sh reports on stderr, stdout empty
        assert_eq!(exec_shell("definitely-not-a-real-binary-xyz"), "");
    }

    #[test]
    fn run_process_propagates_exit_code() {
        assert_eq!(run_process("true", &[]), 0);
        assert_eq!(run_process("false", &[]), 1);
        // FailedToStart -> -1
        assert_eq!(run_process("/definitely/not/a/real/binary", &[]), -1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_argv_matches_oracle_command() {
        let argv = pacman_install_argv(
            &["linux-cachyos".into(), "linux-cachyos-headers".into()],
            true,
        );
        assert_eq!(
            argv,
            vec![
                "pacman",
                "-S",
                "--needed",
                "linux-cachyos",
                "linux-cachyos-headers"
            ]
        );
    }

    #[test]
    fn remove_argv_matches_oracle_command() {
        let argv = pacman_remove_argv(&["linux-cachyos".into()]);
        assert_eq!(argv, vec!["pacman", "-Rsn", "linux-cachyos"]);
    }

    #[test]
    fn makepkg_argv_variants() {
        assert_eq!(
            makepkg_repo_argv(),
            vec!["makepkg", "-scf", "--cleanbuild", "--skipchecksums"]
        );
        assert_eq!(
            makepkg_aur_argv(),
            vec!["makepkg", "-sicf", "--cleanbuild", "--skipchecksums"]
        );
    }

    #[test]
    fn pause_suffix_is_appended() {
        assert_eq!(
            with_pause("pacman -S --needed linux-cachyos"),
            "pacman -S --needed linux-cachyos; read -p 'Press enter to exit'"
        );
    }

    #[test]
    fn finished_proc_success_yes_starts_install() {
        let out = finished_proc(&FinishEvent {
            done_status_exists: true,
            exit_code: 0,
            user_choice: Some(true),
            globs: vec!["linux-cachyos-6.14.1-3-*.pkg.tar.zst".into()],
        });
        assert_eq!(
            out.stdout,
            vec![
                "success",
                "pressed yes",
                "pacman_cmd := sudo pacman -U linux-cachyos-6.14.1-3-*.pkg.tar.zst"
            ]
        );
        assert!(out.stderr.is_empty());
        assert!(out.removes_done_status);
        assert!(out.running_after);
        assert_eq!(
            out.next_command.as_deref(),
            Some("sudo pacman -U linux-cachyos-6.14.1-3-*.pkg.tar.zst")
        );
    }

    #[test]
    fn finished_proc_success_no_does_not_start_install() {
        let out = finished_proc(&FinishEvent {
            done_status_exists: true,
            exit_code: 0,
            user_choice: Some(false),
            globs: vec![],
        });
        assert_eq!(out.stdout, vec!["success"]);
        assert!(out.stderr.is_empty());
        assert!(out.next_command.is_none());
        assert!(out.removes_done_status);
        assert!(!out.running_after);
    }

    #[test]
    fn finished_proc_keys_on_file_not_exit_code() {
        // the .done-status FILE is the success contract: even a nonzero
        // exit code takes the success path when the file exists
        let out = finished_proc(&FinishEvent {
            done_status_exists: true,
            exit_code: 1,
            user_choice: Some(false),
            globs: vec![],
        });
        assert_eq!(out.stdout, vec!["success"]);
        assert!(out.stderr.is_empty());
    }

    #[test]
    fn finished_proc_missing_file_is_failure() {
        // .done-status absent -> stderr message with the exit code (the
        // only use of the exit code), including the oracle's trailing \n
        let out = finished_proc(&FinishEvent {
            done_status_exists: false,
            exit_code: 1,
            user_choice: None,
            globs: vec![],
        });
        assert!(out.stdout.is_empty());
        assert_eq!(
            out.stderr,
            vec!["process failed with exit code: 1\n".to_string()]
        );
        assert!(!out.removes_done_status);
        assert!(!out.running_after);
    }

    #[test]
    fn finished_proc_install_reentry_prints_failure_even_on_success() {
        // the install command runs through the SAME run_cmd_async ->
        // finished_proc; the success path removed .done-status, so the
        // install's OWN completion prints `process failed with exit code: 0`
        // even when pacman succeeded (oracle quirk).
        let build = finished_proc(&FinishEvent {
            done_status_exists: true,
            exit_code: 0,
            user_choice: Some(true),
            globs: vec!["x.pkg.tar.zst".into()],
        });
        assert!(build.next_command.is_some());
        let install = finished_proc(&FinishEvent {
            done_status_exists: false,
            exit_code: 0,
            user_choice: None,
            globs: vec![],
        });
        assert_eq!(
            install.stderr,
            vec!["process failed with exit code: 0\n".to_string()]
        );
    }

    #[test]
    fn configure_trace_guards_double_execute() {
        let (trace, running) = configure_trace(&[
            ConfigureAction::Execute,
            ConfigureAction::Execute,
            ConfigureAction::Cancel,
        ]);
        let outcomes: Vec<&str> = trace.iter().map(|e| e.outcome.as_str()).collect();
        assert_eq!(outcomes, vec!["start", "ignored", "closed"]);
        assert!(!running);
    }

    #[test]
    fn configure_trace_close_is_terminal() {
        let (trace, running) = configure_trace(&[
            ConfigureAction::Execute,
            ConfigureAction::Close,
            ConfigureAction::Execute, // unreachable in the oracle
        ]);
        let outcomes: Vec<&str> = trace.iter().map(|e| e.outcome.as_str()).collect();
        assert_eq!(outcomes, vec!["start", "closed"]);
        assert!(!running);
    }

    #[test]
    fn configure_trace_serializes_lowercase() {
        let json = serde_json::to_string(&ConfigureAction::Execute).unwrap();
        assert_eq!(json, "\"execute\"");
    }

    #[test]
    fn terminal_helper_argv_escalated_matches_oracle() {
        let argv = terminal_helper_argv(
            "pacman -S --needed linux-cachyos",
            Escalate::PkexecRootShell,
        );
        assert_eq!(
            argv,
            vec![
                "/usr/lib/cachyos-kernel-manager/terminal-helper",
                "-s",
                "pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh",
                "pacman -S --needed linux-cachyos; read -p 'Press enter to exit'"
            ]
        );
    }

    #[test]
    fn terminal_helper_argv_non_escalated_has_no_dash_s() {
        let argv =
            terminal_helper_argv("makepkg -scf --cleanbuild --skipchecksums", Escalate::None);
        assert_eq!(
            argv,
            vec![
                "/usr/lib/cachyos-kernel-manager/terminal-helper",
                "makepkg -scf --cleanbuild --skipchecksums; read -p 'Press enter to exit'"
            ]
        );
    }
}
