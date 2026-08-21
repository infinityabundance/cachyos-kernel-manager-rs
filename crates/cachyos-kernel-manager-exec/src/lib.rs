//! Command modeling and argv rendering.
//!
//! The directive's rule: model commands before rendering them; shell
//! construction is business logic and must not leak into domain models.
//! This crate turns [`CommandPlan`] values into concrete argv vectors at the
//! execution boundary, reproducing the oracle's exact chains
//! (`oracle/upstream/src/utils.cpp`, `src/terminal-helper`,
//! `src/conf-window.cpp`).
//!
//! Reconstructed at revision `6b4a373e`; courts: `terminal-helper/*`,
//! `privilege/*`, `transaction-plan/*`.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

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
