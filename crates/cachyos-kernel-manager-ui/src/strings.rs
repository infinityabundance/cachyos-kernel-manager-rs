//! The oracle's user-visible strings — the `ui/dialog-strings` court's
//! candidate side. Every string is quoted from the frozen source with its
//! file reference; the court byte-compares this table against the
//! source-derived reference (`tools/strings-oracle-ref`).
//!
//! i18n note: these are the SOURCE strings (English, the `.ui`/`.cpp`
//! defaults). The translation catalogs (`cachyos-kernel-manager_*.qm`)
//! replace them per locale (the `i18n` crate); the string IDENTITY is the
//! English source text.

#![forbid(unsafe_code)]

/// Window titles.
pub mod titles {
    /// `km-window.ui:17`.
    pub const MAIN: &str = "CachyOS Kernel Manager";
    /// `conf-window.ui:17`.
    pub const CONFIGURE: &str = "CachyOS Kernel Manager Configure";
    /// `schedext-window.ui` (scx-manager at f3eeaf6).
    pub const SCX: &str = "CachyOS Configure sched-ext";
    /// The dialog title used for every `QMessageBox` (`km-window.cpp:144`,
    /// `conf-window.cpp:390`, ...).
    pub const DIALOG: &str = "CachyOS Kernel Manager";
}

/// The main window's description label (`km-window.ui:27`).
pub const MAIN_DESCRIPTION_HTML: &str = "<html>\n<body>\n<p>Here you'll see information about currently installed and available Linux kernels.</p>\n<p>You can install/uninstall kernel packages using the checkboxes on the leftmost column.</p>\n<p>This app won't work if you are already running a pacman instance.</p>\n</body>\n</html>";

/// The main window's description rendered as PLAIN TEXT. The oracle shows
/// the HTML literal above as Qt rich text (`km-window.ui:27`); Slint has no
/// HTML renderer, so the view strips the tags (the courted string
/// inventory keeps the raw HTML literal unchanged — only the presentation
/// differs, which the window-choreography note in `app.rs` declares a
/// rendering choice).
pub fn main_description_plain() -> String {
    strip_html(MAIN_DESCRIPTION_HTML)
}

/// Remove the markup from a small fixed HTML string and keep the paragraph
/// breaks: each `<p>` block becomes its own line (the oracle renders the
/// three sentences as separate Qt-rich-text paragraphs).
fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // the tags leave blank lines; collapse runs of blank lines into ONE
    // newline so the three sentences render as three separate lines
    out.split('\n')
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The kernels tree columns (`km-window.ui:66-81`).
pub mod tree_columns {
    pub const CHOOSE: &str = "Choose";
    pub const PKG_NAME: &str = "PkgName";
    pub const VERSION: &str = "Version";
    pub const CATEGORY: &str = "Category";
}

/// The main-window buttons (`km-window.ui:118-139`).
pub mod main_buttons {
    pub const SCHED_EXT: &str = "sched-ext scheduler config";
    pub const CONFIGURE: &str = "Configure";
    pub const CANCEL: &str = "Cancel";
    pub const EXECUTE: &str = "Execute";
}

/// The Configure window's variant combo labels
/// (`conf-window.cpp:487-496`).
pub const VARIANT_LABELS: [&str; 10] = [
    "CachyOS default Scheduler (tuned EEVDF)",
    "BORE - Burst-Oriented Response Enhancer",
    "RC - Release Candidate",
    "RT - Realtime kernel",
    "LTS - Long-term support kernel",
    "EEVDF",
    "BMQ (BitMap Queue)",
    "Hardened - Hardened Linux kernel",
    "Deckify - Handheld optimized kernel",
    "Server - Server optimized kernel",
];

/// The Configure window's combo options (`conf-window.cpp:503-546`).
pub mod combo_options {
    /// `hzticks_combo_box` — `1000HZ`..`100Hz`.
    pub const HZ_TICKS: [&str; 7] = [
        "1000HZ", "750Hz", "600Hz", "500Hz", "300Hz", "250Hz", "100Hz",
    ];
    /// `tickless_combo_box`.
    pub const TICKLESS: [&str; 3] = ["Full", "Idle", "Periodic"];
    /// `preempt_combo_box` (base set; hardened/lts extend with
    /// Voluntary/None).
    pub const PREEMPT: [&str; 2] = ["Full", "Lazy"];
    /// `processor_opt_combo_box`.
    pub const CPU_OPT: [&str; 7] = [
        "Disabled",
        "Native CPU",
        "Generic / x86_64",
        "x86_64_v2",
        "x86_64_v3",
        "x86_64_v4",
        "Zen4",
    ];
    /// `lto_combo_box`.
    pub const LTO: [&str; 4] = ["No", "Full", "Thin", "Thin-dist"];
    /// `hugepage_combo_box`.
    pub const HUGE_PAGE: [&str; 2] = ["Always", "Madvise"];
    /// The profile combo (`schedext-window-internal.cpp:153-157`).
    pub const SCX_PROFILE: [&str; 5] = ["Auto", "Gaming", "Powersave", "Lowlatency", "Server"];
}

/// Progress-dialog labels (`km-window.cpp:278,342,363`).
pub mod progress {
    pub const INITIALIZING_KERNELS: &str = "Please wait...\nInitializing kernels..";
    pub const PREPARING_CONFIGURATION: &str =
        "Please wait...\nWe are preparing configuration window for you\ncloning PKGBUILDs..";
}

/// `QMessageBox` texts.
pub mod dialogs {
    /// `conf-window.cpp:390` (the build-complete install question).
    pub const INSTALL_BUILD_PACKAGES: &str = "Do you want to install build packages?";
    /// `km-window.cpp:144`.
    pub const FAILED_ALPM_INIT: &str = "Failed to initialize alpm handle (%1)";
    /// `km-window.cpp:157`.
    pub const FAILED_ALPM_RELEASE: &str = "Failed to release alpm handle (%1)";
    /// `km-window.cpp:203`.
    pub const FAILED_CLONE: &str =
        "Failed to clone repository!\nPlease check your internet connection and try again";
    /// `km-window.cpp:229`.
    pub const NO_KERNELS: &str = "No kernels found!\nPlease run `pacman -Sy` to update DB!\nThis is needed for the app to work properly";
    /// `conf-window.cpp:768`.
    pub const FAILED_SAVE_CONFIG: &str = "Failed to save config options to file: %1";
    /// `conf-window.cpp:786`.
    pub const FAILED_LOAD_CONFIG: &str = "Failed to load config options from file: %1";
    /// `conf-window.cpp:808`.
    pub const CONFIG_OUTDATED: &str = "Config file(%1) is outdated";
    /// `conf-window.cpp:618-620` (the local-patch file picker).
    pub const SELECT_PATCH_FILES: &str = "Select one or more patch files";
    pub const PATCH_FILE_FILTER: &str = "Patch file (*.patch)";
    /// `conf-window.cpp:640-641` (the remote-patch URL input).
    pub const ENTER_URL_PATCH: &str = "Enter URL patch";
    pub const PATCH_URL: &str = "Patch URL:";
    /// `conf-window.cpp:759-761` (the save/load file pickers).
    pub const SAVE_FILE_AS: &str = "Save file as";
    pub const LOAD_FROM: &str = "Load from";
    pub const CONFIG_FILE_FILTER: &str = "Config file (*.toml)";
    /// The sched-ext window dialogs (scx-manager at f3eeaf6).
    pub const SCX_CONFIG_INIT: &str = "Cannot initialize scx_loader configuration";
    pub const SCX_NO_LOADER: &str = "Cannot get information from scx_loader!\nIs it working?\nThis is needed for the app to work properly";
    pub const SCX_FLAGS: &str = "Cannot get scx flags from scx_loader configuration!";
    pub const SCX_APPLY: &str =
        "Cannot set default scx scheduler with mode! Scheduler %1 with mode %2";
    pub const SCX_DISABLE: &str = "Cannot disable scx_loader";
}

/// The sched-ext window labels (`schedext-window.ui`).
pub mod scx_labels {
    pub const RUNNING_SCHEDULER: &str = "Running sched-ext scheduler:";
    pub const SELECT_SCHEDULER: &str = "Select sched-ext scheduler:";
    pub const SELECT_PROFILE: &str = "Select scheduler profile:";
    pub const SET_FLAGS: &str = "Set sched-ext extra scheduler flags:";
}

/// stdout lines (`fmt::print` without stderr).
pub mod stdout {
    /// `conf-window.cpp:388`.
    pub const SUCCESS: &str = "success\n";
    /// `conf-window.cpp:392`.
    pub const PRESSED_YES: &str = "pressed yes\n";
    /// `conf-window.cpp:398` (`pacman_cmd := <cmd>`).
    pub const PACMAN_CMD: &str = "pacman_cmd := {}\n";
    /// `km-window.cpp:206`.
    pub const OPERATION_CANCELED: &str = "the operation was canceled!\n";
}

/// stderr lines (`fmt::print(stderr, ...)`).
pub mod stderr {
    /// `conf-window.cpp:232`.
    pub const FAILED_GET_PKGEXT: &str = "failed to get PKGEXT from /etc/makepkg.conf";
    /// `conf-window.cpp:291`.
    pub const BROKEN_PKGBUILD: &str = "broken pkgbuild; pkgver must be present\n";
    /// `conf-window.cpp:403`.
    pub const PROCESS_FAILED: &str = "process failed with exit code: {}\n";
    /// `conf-window.cpp:720`.
    pub const FAILED_INSERT_SOURCE_ARRAY: &str =
        "Failed to insert new source array into pkgbuild\n";
    /// `conf-window.cpp:727`.
    pub const FAILED_SET_CUSTOM_NAME: &str = "Failed to set custom name in pkgbuild\n";
    /// `kernel.cpp:258`.
    pub const AUR_GATE: &str = "Paru and/or AWK are not installed! Disabling AUR kernels support\n";
    /// `km-window.cpp:53`.
    pub const FAILED_ADD_INSTALL: &str = "failed to add package to be installed ({})\n";
    /// `km-window.cpp:65`.
    pub const FAILED_ADD_REMOVE: &str = "failed to add package to be removed ({})\n";
    /// `km-window.cpp:120`.
    pub const WORKER_WAITING: &str = "Waiting... \n";
    /// `utils.cpp:103`.
    pub const POPEN_FAILED: &str = "popen failed! '{}'\n";
    /// `utils.cpp:167`.
    pub const PREPARE_ENTER: &str = "prepare_git_repo: cannot enter '{}': {}\n";
    /// `utils.cpp:183`.
    pub const PREPARE_CLONE: &str = "prepare_git_repo: 'git clone {}' failed\n";
    /// `utils.cpp:194`.
    pub const PREPARE_REFRESH: &str = "prepare_git_repo: failed to refresh checkout at '{}'\n";
    /// `utils.cpp:208`.
    pub const CANNOT_UNSET_ENV: &str = "Cannot unset environment variable!: {}\n";
    /// `utils.cpp:220`.
    pub const CANNOT_SET_ENV: &str = "Cannot set environment variable!: {}\n";
    /// `utils.cpp:62,75`.
    pub const READWHOLEFILE_FAILED: &str = "[READWHOLEFILE] '{}' read failed: {}\n";
    /// `utils.cpp:87`.
    pub const WRITE_TO_FILE_FAILED: &str = "[WRITE_TO_FILE] '{}' open failed: {}\n";
    /// `config-options.cpp:49`.
    pub const FAILED_PARSE_CONFIG: &str = "Failed to parse config file: {}\n";
    /// `config-options.cpp:108`.
    pub const FAILED_WRITE_CONFIG: &str = "Failed to write config file: {}\n";
}

/// The full ordered inventory `(id, source, text)` — the single candidate
/// source of truth for the `ui/dialog-strings` court (rendered by the
/// `cachyos-kernel-manager-strings` witness).
pub fn inventory() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("titles.main", "km-window.ui:17", titles::MAIN),
        ("titles.configure", "conf-window.ui:17", titles::CONFIGURE),
        ("titles.scx", "schedext-window.ui", titles::SCX),
        ("titles.dialog", "km-window.cpp:144", titles::DIALOG),
        ("main.description", "km-window.ui:27", MAIN_DESCRIPTION_HTML),
        ("tree.choose", "km-window.ui:66", tree_columns::CHOOSE),
        ("tree.pkgname", "km-window.ui:71", tree_columns::PKG_NAME),
        ("tree.version", "km-window.ui:76", tree_columns::VERSION),
        ("tree.category", "km-window.ui:81", tree_columns::CATEGORY),
        (
            "button.schedext",
            "km-window.ui:118",
            main_buttons::SCHED_EXT,
        ),
        (
            "button.configure",
            "km-window.ui:125",
            main_buttons::CONFIGURE,
        ),
        ("button.cancel", "km-window.ui:132", main_buttons::CANCEL),
        ("button.execute", "km-window.ui:139", main_buttons::EXECUTE),
        ("variant.0", "conf-window.cpp:487", VARIANT_LABELS[0]),
        ("variant.1", "conf-window.cpp:488", VARIANT_LABELS[1]),
        ("variant.2", "conf-window.cpp:489", VARIANT_LABELS[2]),
        ("variant.3", "conf-window.cpp:490", VARIANT_LABELS[3]),
        ("variant.4", "conf-window.cpp:491", VARIANT_LABELS[4]),
        ("variant.5", "conf-window.cpp:492", VARIANT_LABELS[5]),
        ("variant.6", "conf-window.cpp:493", VARIANT_LABELS[6]),
        ("variant.7", "conf-window.cpp:494", VARIANT_LABELS[7]),
        ("variant.8", "conf-window.cpp:495", VARIANT_LABELS[8]),
        ("variant.9", "conf-window.cpp:496", VARIANT_LABELS[9]),
        (
            "combo.hz.0",
            "conf-window.cpp:504",
            combo_options::HZ_TICKS[0],
        ),
        (
            "combo.hz.1",
            "conf-window.cpp:505",
            combo_options::HZ_TICKS[1],
        ),
        (
            "combo.hz.2",
            "conf-window.cpp:506",
            combo_options::HZ_TICKS[2],
        ),
        (
            "combo.hz.3",
            "conf-window.cpp:507",
            combo_options::HZ_TICKS[3],
        ),
        (
            "combo.hz.4",
            "conf-window.cpp:508",
            combo_options::HZ_TICKS[4],
        ),
        (
            "combo.hz.5",
            "conf-window.cpp:509",
            combo_options::HZ_TICKS[5],
        ),
        (
            "combo.hz.6",
            "conf-window.cpp:510",
            combo_options::HZ_TICKS[6],
        ),
        (
            "combo.tickless.0",
            "conf-window.cpp:514",
            combo_options::TICKLESS[0],
        ),
        (
            "combo.tickless.1",
            "conf-window.cpp:515",
            combo_options::TICKLESS[1],
        ),
        (
            "combo.tickless.2",
            "conf-window.cpp:516",
            combo_options::TICKLESS[2],
        ),
        (
            "combo.preempt.0",
            "conf-window.cpp:520",
            combo_options::PREEMPT[0],
        ),
        (
            "combo.preempt.1",
            "conf-window.cpp:521",
            combo_options::PREEMPT[1],
        ),
        ("combo.preempt.2", "conf-window.cpp:579", "Voluntary"),
        ("combo.preempt.3", "conf-window.cpp:580", "None"),
        (
            "combo.cpu.0",
            "conf-window.cpp:526",
            combo_options::CPU_OPT[0],
        ),
        (
            "combo.cpu.1",
            "conf-window.cpp:527",
            combo_options::CPU_OPT[1],
        ),
        (
            "combo.cpu.2",
            "conf-window.cpp:528",
            combo_options::CPU_OPT[2],
        ),
        (
            "combo.cpu.3",
            "conf-window.cpp:529",
            combo_options::CPU_OPT[3],
        ),
        (
            "combo.cpu.4",
            "conf-window.cpp:529",
            combo_options::CPU_OPT[4],
        ),
        (
            "combo.cpu.5",
            "conf-window.cpp:529",
            combo_options::CPU_OPT[5],
        ),
        (
            "combo.cpu.6",
            "conf-window.cpp:530",
            combo_options::CPU_OPT[6],
        ),
        ("combo.lto.0", "conf-window.cpp:535", combo_options::LTO[0]),
        ("combo.lto.1", "conf-window.cpp:536", combo_options::LTO[1]),
        ("combo.lto.2", "conf-window.cpp:537", combo_options::LTO[2]),
        ("combo.lto.3", "conf-window.cpp:538", combo_options::LTO[3]),
        (
            "combo.hugepage.0",
            "conf-window.cpp:544",
            combo_options::HUGE_PAGE[0],
        ),
        (
            "combo.hugepage.1",
            "conf-window.cpp:545",
            combo_options::HUGE_PAGE[1],
        ),
        (
            "combo.scx_profile.0",
            "schedext-window-internal.cpp:153",
            combo_options::SCX_PROFILE[0],
        ),
        (
            "combo.scx_profile.1",
            "schedext-window-internal.cpp:154",
            combo_options::SCX_PROFILE[1],
        ),
        (
            "combo.scx_profile.2",
            "schedext-window-internal.cpp:155",
            combo_options::SCX_PROFILE[2],
        ),
        (
            "combo.scx_profile.3",
            "schedext-window-internal.cpp:156",
            combo_options::SCX_PROFILE[3],
        ),
        (
            "combo.scx_profile.4",
            "schedext-window-internal.cpp:157",
            combo_options::SCX_PROFILE[4],
        ),
        (
            "progress.initializing",
            "km-window.cpp:363",
            progress::INITIALIZING_KERNELS,
        ),
        (
            "progress.preparing",
            "km-window.cpp:278",
            progress::PREPARING_CONFIGURATION,
        ),
        (
            "dialog.install_build_packages",
            "conf-window.cpp:390",
            dialogs::INSTALL_BUILD_PACKAGES,
        ),
        (
            "dialog.failed_alpm_init",
            "km-window.cpp:144",
            dialogs::FAILED_ALPM_INIT,
        ),
        (
            "dialog.failed_alpm_release",
            "km-window.cpp:157",
            dialogs::FAILED_ALPM_RELEASE,
        ),
        (
            "dialog.failed_clone",
            "km-window.cpp:203",
            dialogs::FAILED_CLONE,
        ),
        (
            "dialog.no_kernels",
            "km-window.cpp:229",
            dialogs::NO_KERNELS,
        ),
        (
            "dialog.failed_save_config",
            "conf-window.cpp:768",
            dialogs::FAILED_SAVE_CONFIG,
        ),
        (
            "dialog.failed_load_config",
            "conf-window.cpp:786",
            dialogs::FAILED_LOAD_CONFIG,
        ),
        (
            "dialog.config_outdated",
            "conf-window.cpp:808",
            dialogs::CONFIG_OUTDATED,
        ),
        (
            "dialog.select_patch_files",
            "conf-window.cpp:618",
            dialogs::SELECT_PATCH_FILES,
        ),
        (
            "dialog.patch_file_filter",
            "conf-window.cpp:620",
            dialogs::PATCH_FILE_FILTER,
        ),
        (
            "dialog.enter_url_patch",
            "conf-window.cpp:640",
            dialogs::ENTER_URL_PATCH,
        ),
        (
            "dialog.patch_url",
            "conf-window.cpp:641",
            dialogs::PATCH_URL,
        ),
        (
            "dialog.save_file_as",
            "conf-window.cpp:759",
            dialogs::SAVE_FILE_AS,
        ),
        (
            "dialog.load_from",
            "conf-window.cpp:776",
            dialogs::LOAD_FROM,
        ),
        (
            "dialog.config_file_filter",
            "conf-window.cpp:761",
            dialogs::CONFIG_FILE_FILTER,
        ),
        (
            "dialog.scx_config_init",
            "schedext-window-internal.cpp:126",
            dialogs::SCX_CONFIG_INIT,
        ),
        (
            "dialog.scx_no_loader",
            "schedext-window-internal.cpp:140",
            dialogs::SCX_NO_LOADER,
        ),
        (
            "dialog.scx_flags",
            "schedext-window-internal.cpp:205",
            dialogs::SCX_FLAGS,
        ),
        (
            "dialog.scx_apply",
            "schedext-window-internal.cpp:277",
            dialogs::SCX_APPLY,
        ),
        (
            "dialog.scx_disable",
            "schedext-window-internal.cpp:187",
            dialogs::SCX_DISABLE,
        ),
        (
            "scx_label.running",
            "schedext-window.ui",
            scx_labels::RUNNING_SCHEDULER,
        ),
        (
            "scx_label.select_scheduler",
            "schedext-window.ui",
            scx_labels::SELECT_SCHEDULER,
        ),
        (
            "scx_label.select_profile",
            "schedext-window.ui",
            scx_labels::SELECT_PROFILE,
        ),
        (
            "scx_label.set_flags",
            "schedext-window.ui",
            scx_labels::SET_FLAGS,
        ),
        ("stdout.success", "conf-window.cpp:388", stdout::SUCCESS),
        (
            "stdout.pressed_yes",
            "conf-window.cpp:392",
            stdout::PRESSED_YES,
        ),
        (
            "stdout.pacman_cmd",
            "conf-window.cpp:398",
            stdout::PACMAN_CMD,
        ),
        (
            "stdout.operation_canceled",
            "km-window.cpp:206",
            stdout::OPERATION_CANCELED,
        ),
        (
            "stderr.failed_get_pkgext",
            "conf-window.cpp:232",
            stderr::FAILED_GET_PKGEXT,
        ),
        (
            "stderr.broken_pkgbuild",
            "conf-window.cpp:291",
            stderr::BROKEN_PKGBUILD,
        ),
        (
            "stderr.process_failed",
            "conf-window.cpp:403",
            stderr::PROCESS_FAILED,
        ),
        (
            "stderr.failed_insert_source_array",
            "conf-window.cpp:720",
            stderr::FAILED_INSERT_SOURCE_ARRAY,
        ),
        (
            "stderr.failed_set_custom_name",
            "conf-window.cpp:727",
            stderr::FAILED_SET_CUSTOM_NAME,
        ),
        ("stderr.aur_gate", "kernel.cpp:258", stderr::AUR_GATE),
        (
            "stderr.failed_add_install",
            "km-window.cpp:53",
            stderr::FAILED_ADD_INSTALL,
        ),
        (
            "stderr.failed_add_remove",
            "km-window.cpp:65",
            stderr::FAILED_ADD_REMOVE,
        ),
        (
            "stderr.worker_waiting",
            "km-window.cpp:120",
            stderr::WORKER_WAITING,
        ),
        ("stderr.popen_failed", "utils.cpp:103", stderr::POPEN_FAILED),
        (
            "stderr.prepare_enter",
            "utils.cpp:167",
            stderr::PREPARE_ENTER,
        ),
        (
            "stderr.prepare_clone",
            "utils.cpp:183",
            stderr::PREPARE_CLONE,
        ),
        (
            "stderr.prepare_refresh",
            "utils.cpp:194",
            stderr::PREPARE_REFRESH,
        ),
        (
            "stderr.cannot_unset_env",
            "utils.cpp:208",
            stderr::CANNOT_UNSET_ENV,
        ),
        (
            "stderr.cannot_set_env",
            "utils.cpp:220",
            stderr::CANNOT_SET_ENV,
        ),
        (
            "stderr.readwholefile_failed",
            "utils.cpp:62",
            stderr::READWHOLEFILE_FAILED,
        ),
        (
            "stderr.write_to_file_failed",
            "utils.cpp:87",
            stderr::WRITE_TO_FILE_FAILED,
        ),
        (
            "stderr.failed_parse_config",
            "config-options.cpp:49",
            stderr::FAILED_PARSE_CONFIG,
        ),
        (
            "stderr.failed_write_config",
            "config-options.cpp:108",
            stderr::FAILED_WRITE_CONFIG,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_plain_text_drops_the_markup() {
        let plain = main_description_plain();
        assert!(!plain.contains('<'));
        assert!(!plain.contains("</p>"));
        assert!(plain.contains(
            "Here you'll see information about currently installed and available Linux kernels."
        ));
        assert!(plain.contains("You can install/uninstall kernel packages using the checkboxes on the leftmost column."));
        assert!(plain.contains("This app won't work if you are already running a pacman instance."));
        // the three paragraphs render as three separate lines
        assert_eq!(plain.lines().count(), 3);
        assert!(!plain.contains("\n\n"));
    }

    #[test]
    fn raw_description_literal_is_unchanged_for_the_inventory_court() {
        // the courted inventory compares the RAW HTML literal against the
        // oracle's km-window.ui:27 string — it must stay byte-exact
        assert!(MAIN_DESCRIPTION_HTML.starts_with("<html>"));
        assert!(MAIN_DESCRIPTION_HTML.ends_with("</html>"));
        assert!(MAIN_DESCRIPTION_HTML.contains("<p>"));
    }
}
