//! Sysfs state readback — `get_current_scheduler` and the
//! `read_kernel_file` semantics (`scx-manager/src/schedext-window-internal.cpp:38-72`).
//!
//! The 1s timer refreshes the "Running sched-ext scheduler:" label even
//! without scx_loader — this surface is the kernel's own reporting.

/// The sysfs state file (`km-window.cpp:186`, `schedext-window-internal.cpp:62`).
pub const SCHED_EXT_STATE_FILE: &str = "/sys/kernel/sched_ext/state";
/// The running scheduler's ops file (`schedext-window-internal.cpp:66`).
pub const SCHED_EXT_OPS_FILE: &str = "/sys/kernel/sched_ext/root/ops";

/// `read_kernel_file` (`schedext-window-internal.cpp:39-55`): first line of
/// the file; `None` when the file cannot be opened (no message); the
/// "Failed to read := '<path>'" stderr line when the file opens but the
/// first `getline` fails (empty/unreadable content).
pub fn read_kernel_file(
    file_path: &str,
    opens: bool,
    getline_ok: bool,
    first_line: &str,
) -> (String, Option<String>) {
    if !opens {
        return (String::new(), None);
    }
    if !getline_ok {
        return (
            String::new(),
            Some(format!("Failed to read := '{file_path}'\n")),
        );
    }
    (first_line.to_string(), None)
}

/// `get_current_scheduler` (`schedext-window-internal.cpp:57-72`):
/// - state != `"enabled"` → the state text itself (e.g. `"disabled"`);
/// - state == `"enabled"` and the ops read is empty → `"unknown"`;
/// - otherwise the ops text (the running scheduler).
pub fn current_scheduler(state_contents: &str, ops_contents: &str) -> String {
    if state_contents != "enabled" {
        return state_contents.to_string();
    }
    if ops_contents.is_empty() {
        return "unknown".to_string();
    }
    ops_contents.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_not_enabled_is_reported_verbatim() {
        assert_eq!(current_scheduler("disabled", "scx_bpfland"), "disabled");
        assert_eq!(current_scheduler("enabledx", ""), "enabledx");
        assert_eq!(current_scheduler("", ""), "");
    }

    #[test]
    fn enabled_without_ops_is_unknown() {
        assert_eq!(current_scheduler("enabled", ""), "unknown");
    }

    #[test]
    fn enabled_with_ops_reports_the_scheduler() {
        assert_eq!(current_scheduler("enabled", "scx_bpfland"), "scx_bpfland");
    }

    #[test]
    fn read_kernel_file_semantics() {
        assert_eq!(
            read_kernel_file("/sys/kernel/sched_ext/state", true, true, "enabled"),
            ("enabled".to_string(), None)
        );
        let (content, msg) = read_kernel_file("/x", false, false, "");
        assert_eq!(content, "");
        assert_eq!(msg, None);
        let (content, msg) = read_kernel_file("/x", true, false, "");
        assert_eq!(content, "");
        assert_eq!(msg, Some("Failed to read := '/x'\n".to_string()));
    }
}
