//! Reference re-declaration of the ORACLE's user-visible strings for the
//! `ui/dialog-strings` court (`cachyos-km-strings-v1`). Every row is quoted
//! from the frozen source files at `oracle/upstream/src/` (revision
//! `6b4a373e`; the sched-ext strings from the pre-extraction scx-manager at
//! `f3eeaf6`, `oracle/scx-authority/`). Written INDEPENDENTLY from the
//! candidate's `strings.rs` — the court catches any drift between the two
//! hand-writings of the same authority.
//!
//! This tool is court evidence infrastructure, never shipped.

use serde_json::json;
use std::process::ExitCode;

const ROWS: &[(&str, &str, &str)] = &[
    ("titles.main", "km-window.ui:17", "CachyOS Kernel Manager"),
    ("titles.configure", "conf-window.ui:17", "CachyOS Kernel Manager Configure"),
    ("titles.scx", "schedext-window.ui", "CachyOS Configure sched-ext"),
    ("titles.dialog", "km-window.cpp:144", "CachyOS Kernel Manager"),
    ("main.description", "km-window.ui:27", "<html>\n<body>\n<p>Here you'll see information about currently installed and available Linux kernels.</p>\n<p>You can install/uninstall kernel packages using the checkboxes on the leftmost column.</p>\n<p>This app won't work if you are already running a pacman instance.</p>\n</body>\n</html>"),
    ("tree.choose", "km-window.ui:66", "Choose"),
    ("tree.pkgname", "km-window.ui:71", "PkgName"),
    ("tree.version", "km-window.ui:76", "Version"),
    ("tree.category", "km-window.ui:81", "Category"),
    ("button.schedext", "km-window.ui:118", "sched-ext scheduler config"),
    ("button.configure", "km-window.ui:125", "Configure"),
    ("button.cancel", "km-window.ui:132", "Cancel"),
    ("button.execute", "km-window.ui:139", "Execute"),
    ("variant.0", "conf-window.cpp:487", "CachyOS default Scheduler (tuned EEVDF)"),
    ("variant.1", "conf-window.cpp:488", "BORE - Burst-Oriented Response Enhancer"),
    ("variant.2", "conf-window.cpp:489", "RC - Release Candidate"),
    ("variant.3", "conf-window.cpp:490", "RT - Realtime kernel"),
    ("variant.4", "conf-window.cpp:491", "LTS - Long-term support kernel"),
    ("variant.5", "conf-window.cpp:492", "EEVDF"),
    ("variant.6", "conf-window.cpp:493", "BMQ (BitMap Queue)"),
    ("variant.7", "conf-window.cpp:494", "Hardened - Hardened Linux kernel"),
    ("variant.8", "conf-window.cpp:495", "Deckify - Handheld optimized kernel"),
    ("variant.9", "conf-window.cpp:496", "Server - Server optimized kernel"),
    ("combo.hz.0", "conf-window.cpp:504", "1000HZ"),
    ("combo.hz.1", "conf-window.cpp:505", "750Hz"),
    ("combo.hz.2", "conf-window.cpp:506", "600Hz"),
    ("combo.hz.3", "conf-window.cpp:507", "500Hz"),
    ("combo.hz.4", "conf-window.cpp:508", "300Hz"),
    ("combo.hz.5", "conf-window.cpp:509", "250Hz"),
    ("combo.hz.6", "conf-window.cpp:510", "100Hz"),
    ("combo.tickless.0", "conf-window.cpp:514", "Full"),
    ("combo.tickless.1", "conf-window.cpp:515", "Idle"),
    ("combo.tickless.2", "conf-window.cpp:516", "Periodic"),
    ("combo.preempt.0", "conf-window.cpp:520", "Full"),
    ("combo.preempt.1", "conf-window.cpp:521", "Lazy"),
    ("combo.preempt.2", "conf-window.cpp:579", "Voluntary"),
    ("combo.preempt.3", "conf-window.cpp:580", "None"),
    ("combo.cpu.0", "conf-window.cpp:526", "Disabled"),
    ("combo.cpu.1", "conf-window.cpp:527", "Native CPU"),
    ("combo.cpu.2", "conf-window.cpp:528", "Generic / x86_64"),
    ("combo.cpu.3", "conf-window.cpp:529", "x86_64_v2"),
    ("combo.cpu.4", "conf-window.cpp:529", "x86_64_v3"),
    ("combo.cpu.5", "conf-window.cpp:529", "x86_64_v4"),
    ("combo.cpu.6", "conf-window.cpp:530", "Zen4"),
    ("combo.lto.0", "conf-window.cpp:535", "No"),
    ("combo.lto.1", "conf-window.cpp:536", "Full"),
    ("combo.lto.2", "conf-window.cpp:537", "Thin"),
    ("combo.lto.3", "conf-window.cpp:538", "Thin-dist"),
    ("combo.hugepage.0", "conf-window.cpp:544", "Always"),
    ("combo.hugepage.1", "conf-window.cpp:545", "Madvise"),
    ("combo.scx_profile.0", "schedext-window-internal.cpp:153", "Auto"),
    ("combo.scx_profile.1", "schedext-window-internal.cpp:154", "Gaming"),
    ("combo.scx_profile.2", "schedext-window-internal.cpp:155", "Powersave"),
    ("combo.scx_profile.3", "schedext-window-internal.cpp:156", "Lowlatency"),
    ("combo.scx_profile.4", "schedext-window-internal.cpp:157", "Server"),
    ("progress.initializing", "km-window.cpp:363", "Please wait...\nInitializing kernels.."),
    ("progress.preparing", "km-window.cpp:278", "Please wait...\nWe are preparing configuration window for you\ncloning PKGBUILDs.."),
    ("dialog.install_build_packages", "conf-window.cpp:390", "Do you want to install build packages?"),
    ("dialog.failed_alpm_init", "km-window.cpp:144", "Failed to initialize alpm handle (%1)"),
    ("dialog.failed_alpm_release", "km-window.cpp:157", "Failed to release alpm handle (%1)"),
    ("dialog.failed_clone", "km-window.cpp:203", "Failed to clone repository!\nPlease check your internet connection and try again"),
    ("dialog.no_kernels", "km-window.cpp:229", "No kernels found!\nPlease run `pacman -Sy` to update DB!\nThis is needed for the app to work properly"),
    ("dialog.failed_save_config", "conf-window.cpp:768", "Failed to save config options to file: %1"),
    ("dialog.failed_load_config", "conf-window.cpp:786", "Failed to load config options from file: %1"),
    ("dialog.config_outdated", "conf-window.cpp:808", "Config file(%1) is outdated"),
    ("dialog.select_patch_files", "conf-window.cpp:618", "Select one or more patch files"),
    ("dialog.patch_file_filter", "conf-window.cpp:620", "Patch file (*.patch)"),
    ("dialog.enter_url_patch", "conf-window.cpp:640", "Enter URL patch"),
    ("dialog.patch_url", "conf-window.cpp:641", "Patch URL:"),
    ("dialog.save_file_as", "conf-window.cpp:759", "Save file as"),
    ("dialog.load_from", "conf-window.cpp:776", "Load from"),
    ("dialog.config_file_filter", "conf-window.cpp:761", "Config file (*.toml)"),
    ("dialog.scx_config_init", "schedext-window-internal.cpp:126", "Cannot initialize scx_loader configuration"),
    ("dialog.scx_no_loader", "schedext-window-internal.cpp:140", "Cannot get information from scx_loader!\nIs it working?\nThis is needed for the app to work properly"),
    ("dialog.scx_flags", "schedext-window-internal.cpp:205", "Cannot get scx flags from scx_loader configuration!"),
    ("dialog.scx_apply", "schedext-window-internal.cpp:277", "Cannot set default scx scheduler with mode! Scheduler %1 with mode %2"),
    ("dialog.scx_disable", "schedext-window-internal.cpp:187", "Cannot disable scx_loader"),
    ("scx_label.running", "schedext-window.ui", "Running sched-ext scheduler:"),
    ("scx_label.select_scheduler", "schedext-window.ui", "Select sched-ext scheduler:"),
    ("scx_label.select_profile", "schedext-window.ui", "Select scheduler profile:"),
    ("scx_label.set_flags", "schedext-window.ui", "Set sched-ext extra scheduler flags:"),
    ("stdout.success", "conf-window.cpp:388", "success\n"),
    ("stdout.pressed_yes", "conf-window.cpp:392", "pressed yes\n"),
    ("stdout.pacman_cmd", "conf-window.cpp:398", "pacman_cmd := {}\n"),
    ("stdout.operation_canceled", "km-window.cpp:206", "the operation was canceled!\n"),
    ("stderr.failed_get_pkgext", "conf-window.cpp:232", "failed to get PKGEXT from /etc/makepkg.conf"),
    ("stderr.broken_pkgbuild", "conf-window.cpp:291", "broken pkgbuild; pkgver must be present\n"),
    ("stderr.process_failed", "conf-window.cpp:403", "process failed with exit code: {}\n"),
    ("stderr.failed_insert_source_array", "conf-window.cpp:720", "Failed to insert new source array into pkgbuild\n"),
    ("stderr.failed_set_custom_name", "conf-window.cpp:727", "Failed to set custom name in pkgbuild\n"),
    ("stderr.aur_gate", "kernel.cpp:258", "Paru and/or AWK are not installed! Disabling AUR kernels support\n"),
    ("stderr.failed_add_install", "km-window.cpp:53", "failed to add package to be installed ({})\n"),
    ("stderr.failed_add_remove", "km-window.cpp:65", "failed to add package to be removed ({})\n"),
    ("stderr.worker_waiting", "km-window.cpp:120", "Waiting... \n"),
    ("stderr.popen_failed", "utils.cpp:103", "popen failed! '{}'\n"),
    ("stderr.prepare_enter", "utils.cpp:167", "prepare_git_repo: cannot enter '{}': {}\n"),
    ("stderr.prepare_clone", "utils.cpp:183", "prepare_git_repo: 'git clone {}' failed\n"),
    ("stderr.prepare_refresh", "utils.cpp:194", "prepare_git_repo: failed to refresh checkout at '{}'\n"),
    ("stderr.cannot_unset_env", "utils.cpp:208", "Cannot unset environment variable!: {}\n"),
    ("stderr.cannot_set_env", "utils.cpp:220", "Cannot set environment variable!: {}\n"),
    ("stderr.readwholefile_failed", "utils.cpp:62", "[READWHOLEFILE] '{}' read failed: {}\n"),
    ("stderr.write_to_file_failed", "utils.cpp:87", "[WRITE_TO_FILE] '{}' open failed: {}\n"),
    ("stderr.failed_parse_config", "config-options.cpp:49", "Failed to parse config file: {}\n"),
    ("stderr.failed_write_config", "config-options.cpp:108", "Failed to write config file: {}\n"),
];

fn main() -> ExitCode {
    let rows: Vec<serde_json::Value> = ROWS
        .iter()
        .map(|(id, source, text)| json!({ "id": id, "source": source, "text": text }))
        .collect();
    let payload = json!({ "schema": "cachyos-km-strings-v1", "strings": rows });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
