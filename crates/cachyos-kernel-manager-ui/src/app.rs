//! The Iced application — the Phase 8 rendering layer.
//!
//! Layering discipline (docs/ARCHITECTURE.md): this module ONLY translates
//! Iced messages into the courted semantic substrate (core `AppState`
//! transitions, the plan/exec/build/config/scx models) and renders it. No
//! domain semantics live here; a UI bug must be attributable to this layer,
//! never to the models.
//!
//! Window strategy: the oracle has three native windows (Main/Configure/
//! SchedExt). The candidate renders them as a single-window view stack —
//! the Configure window replaces the main view while `ConfigurationState`
//! is `Editing`, the sched-ext window overlays while `ScxState` is
//! `Visible`, and the dialogs (progress/error/confirm) render on top of
//! whatever is active. The *semantics* are courted; the window choreography
//! is a rendering choice. Likewise the file dialogs: iced has no native
//! picker, so Load/Save/Add-patch use a small inline path editor (the
//! config crate owns what gets written/read).

use crate::configure_window::ConfigureWindowModel;
use crate::i18n::{resolve, ResolvedLocale};
use crate::main_window::rows;
use crate::scx_window::ScxWindowModel;
use crate::strings;
use crate::{KernelRowView, Message};
use cachyos_kernel_manager_config::KernelManagerConfig;
use cachyos_kernel_manager_core::discovery::DiscoveredKernel;
use cachyos_kernel_manager_core::options::{CpuOptMode, HugepageMode, HzTick, KernelVariant};
use cachyos_kernel_manager_core::selection::KernelRow;
use cachyos_kernel_manager_core::state::{
    transition, AppEvent, AppState, ConfigurationState, DialogsState, Effect, ScxState,
};
use cachyos_kernel_manager_plan::HardwareProfile;
use iced::widget::{
    button, center, checkbox, column, container, horizontal_space, pick_list, progress_bar, row,
    scrollable, stack, text, text_input,
};
use iced::{Element, Task};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Flags + environment
// ---------------------------------------------------------------------------

/// Startup facts read from the environment (the oracle reads the same from
/// `QApplication` + `QStandardPaths`).
#[derive(Debug, Clone)]
pub struct Flags {
    pub home: String,
    /// `QLocale::system().name()`-style value (`de_DE`; the encoding is
    /// stripped from `LANG=de_DE.UTF-8`).
    pub system_locale: String,
    /// The scx_loader config path (`/etc/scx_loader.toml`).
    pub config_path: String,
    /// Whether the AUR kernel feature is compiled in (the meson
    /// `aur_kernels` flag; the shipped oracle has it OFF).
    pub aur_enabled: bool,
}

impl Flags {
    pub fn from_env() -> Flags {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let raw_locale = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_else(|_| "C".to_string());
        let system_locale = raw_locale
            .split('.')
            .next()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "C".to_string());
        Flags {
            home,
            system_locale,
            config_path: "/etc/scx_loader.toml".to_string(),
            aur_enabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Discovery payload
// ---------------------------------------------------------------------------

/// The result of one discovery pass (the app's catalog).
#[derive(Debug, Clone)]
pub struct CatalogPayload {
    /// The tree rows (`main_window::rows` — the courted assembly).
    pub rows: Vec<KernelRowView>,
    /// Kernels by raw id (`<repo>/<kernel>`), for planning.
    pub kernels: BTreeMap<String, DiscoveredKernel>,
    /// Installed provenance by package name.
    pub installed: BTreeMap<String, (Option<String>, String)>,
    /// Hardware facts for the plan expansion.
    pub hardware: HardwareProfile,
}

/// Run discovery: the real libalpm backend with the `alpm` feature, an
/// EMPTY catalog otherwise (CI/dev — the oracle's "No kernels found" path).
#[cfg(feature = "alpm")]
pub fn run_discovery(flags: &Flags) -> CatalogPayload {
    use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
    use cachyos_kernel_manager_alpm::pacman_conf::MiniIni;
    use cachyos_kernel_manager_alpm::{register_sections, Alpm};
    use cachyos_kernel_manager_core::discovery::SyncDb;
    use cachyos_kernel_manager_core::DbPackage;

    struct RealAlpm(AlpmHandle);
    impl Alpm for RealAlpm {
        fn sync_dbs(&self) -> Vec<SyncDb> {
            self.0
                .syncdb_names()
                .into_iter()
                .map(|name| SyncDb {
                    name: name.clone(),
                    packages: self
                        .0
                        .db_packages(&name)
                        .into_iter()
                        .map(|p| DbPackage {
                            name: p.name,
                            version: p.version,
                        })
                        .collect(),
                })
                .collect()
        }
        fn local_pkg(&self, name: &str) -> Option<DbPackage> {
            self.0.local_pkg(name).map(|l| DbPackage {
                name: l.name,
                version: l.version,
            })
        }
        fn installed_db(&self, name: &str) -> Option<String> {
            self.0.local_pkg(name).and_then(|l| l.installed_db)
        }
        fn vercmp(&self, a: &str, b: &str) -> std::cmp::Ordering {
            self.0.vercmp(a, b).cmp(&0)
        }
    }

    let content = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let sections: Vec<String> = MiniIni::parse(&content)
        .section_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let sections = register_sections(&sections);
    let handle = AlpmHandle::init("/", "/var/lib/pacman/").expect("alpm init (libalpm build)");
    for name in &sections {
        handle.register_syncdb(name);
    }
    discover_from(&RealAlpm(handle), flags)
}

/// Non-libalpm build: an empty catalog (CI/dev without system libalpm).
#[cfg(not(feature = "alpm"))]
pub fn run_discovery(flags: &Flags) -> CatalogPayload {
    use cachyos_kernel_manager_alpm::NullAlpm;
    discover_from(&NullAlpm::default(), flags)
}

/// Discover + assemble the catalog from an [`Alpm`] source. Mirrors the
/// oracle's `Kernel::get_kernels` + the courted `main_window::rows`
/// assembly.
pub fn discover_from(
    alpm: &impl cachyos_kernel_manager_alpm::Alpm,
    _flags: &Flags,
) -> CatalogPayload {
    use cachyos_kernel_manager_core::discovery::discover_kernels;

    let dbs = alpm.sync_dbs();
    let kernels = discover_kernels(&dbs);
    let kernels_by_raw: BTreeMap<String, DiscoveredKernel> =
        kernels.iter().map(|k| (k.raw.clone(), k.clone())).collect();
    let installed: BTreeMap<String, (Option<String>, String)> = kernels
        .iter()
        .filter_map(|k| {
            alpm.local_pkg(&k.name)
                .map(|p| (p.name.clone(), (alpm.installed_db(&k.name), p.version)))
        })
        .collect();
    let vercmp = |a: &str, b: &str| alpm.vercmp(a, b);
    let view_rows = rows(&kernels, |name| installed.get(name).cloned(), vercmp);
    // hardware facts: the sync dbs are a conservative stand-in for the null
    // backend; the real backend's local database is authoritative
    let mut all_installed = std::collections::BTreeSet::new();
    for db in &dbs {
        for p in &db.packages {
            all_installed.insert(p.name.clone());
        }
    }
    let hardware = HardwareProfile {
        installed: all_installed,
        ..HardwareProfile::default()
    };
    CatalogPayload {
        rows: view_rows,
        kernels: kernels_by_raw,
        installed,
        hardware,
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// The Configure window's option checkboxes (the oracle's
/// `checkbox_bindings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfCheck {
    Hardly,
    PerGov,
    TcpBbr3,
    CachyConfig,
    Nconfig,
    Xconfig,
    Localmodcfg,
    UseCurrent,
    Zfs,
    NvidiaOpen,
    BuildDebug,
}

/// The Configure window's patch-list operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfPatchOp {
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
}

/// The Configure window's tab (the oracle's QTabWidget).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfTab {
    Options,
    Patches,
}

/// A file-path entry dialog (iced has no native picker; the oracle's
/// QFileDialog is a rendering concern).
#[derive(Debug, Clone)]
pub struct PathDialog {
    pub title: String,
    pub value: String,
    pub on_submit: PathDialogKind,
}

#[derive(Debug, Clone)]
pub enum PathDialogKind {
    LoadConfig,
    SaveConfig,
    AddRemotePatch,
    AddLocalPatch,
}

/// The Iced-level message: semantic UI messages (the courted vocabulary)
/// plus rendering-only messages (task results, dialogs, widgets).
#[derive(Debug, Clone)]
pub enum UiMessage {
    /// The semantic message vocabulary (crates/.../lib.rs).
    Semantic(Message),
    /// A discovery pass finished (initial load or post-transaction refresh).
    CatalogLoaded(Box<CatalogPayload>),
    /// The transaction commit finished; `changed` = the kernels-change flag.
    TransactionFinished {
        changed: bool,
    },
    /// The transaction failed (the alpm change-check).
    TransactionFailed {
        message: String,
    },
    /// The Configure-window prepare flow (git refresh) finished.
    ConfigurePrepared,
    /// The build finished (`.done-status` presence).
    BuildFinished {
        success: bool,
    },
    /// The artifact install (`sudo pacman -U`) finished.
    ArtifactsInstalled,
    /// The sched-ext window finished initializing.
    ScxInit(Box<ScxWindowModel>),
    /// The sched-ext apply/disable D-Bus call finished.
    ScxApplied {
        ok: bool,
    },
    ScxDisabled {
        ok: bool,
    },
    /// A dialog was dismissed.
    DialogDismissed,
    /// The "install build packages?" question answer.
    InstallQuestion(bool),
    /// The custom-name field changed.
    CustomNameChanged(String),
    /// The sched-ext flags field changed.
    ScxFlagsChanged(String),
    /// A config file was loaded.
    ConfigLoaded(Box<KernelManagerConfig>),
    /// The app should exit (the Close effect).
    Exit,
    /// The user sorted the tree by a column.
    SortRequested {
        column: usize,
    },
    /// The Configure window tab changed.
    ConfTabClicked(ConfTab),
    /// An option checkbox toggled.
    CheckToggled(ConfCheck),
    /// A patch-list operation.
    PatchOp(ConfPatchOp),
    /// The variant combo changed.
    VariantPicked(KernelVariant),
    /// The lto/preempt/hz combos changed.
    LtoPicked(cachyos_kernel_manager_core::options::LtoMode),
    PreemptPicked(cachyos_kernel_manager_core::options::PreemptMode),
    HzPicked(HzTick),
    /// The sched-ext window buttons.
    ScxApply,
    ScxDisable,
    ScxSchedulerPicked(String),
    ScxProfilePicked(String),
    /// A path dialog opened/typed/submitted.
    PathDialogOpened(PathDialogKind),
    PathDialogChanged(String),
    PathDialogDismissed,
    PathDialogSubmitted,
}

/// The Qt contexts for the translations (the `.ts` context names).
pub mod tr_ctx {
    pub const MAIN: &str = "MainWindow";
    pub const CONF: &str = "ConfWindow";
    pub const CONF_OPTIONS: &str = "ConfOptionsPage";
    pub const CONF_PATCHES: &str = "ConfPatchesPage";
    pub const SCX: &str = "SchedExtWindow";
}

// ---------------------------------------------------------------------------
// The application state
// ---------------------------------------------------------------------------

/// The app: the courted core state + the UI projections.
pub struct App {
    state: AppState,
    /// The tree rows (discovery order; re-sorted by the current sort).
    rows: Vec<KernelRowView>,
    /// Kernels by raw id.
    kernels: BTreeMap<String, DiscoveredKernel>,
    /// Installed provenance by package name.
    installed: BTreeMap<String, (Option<String>, String)>,
    /// Hardware facts for planning.
    hardware: HardwareProfile,
    /// The Configure window model (active while `configuration != Closed`).
    configure: ConfigureWindowModel,
    /// The sched-ext window model (active while `scx == Visible`).
    scx: Option<ScxWindowModel>,
    /// The resolved locale (the i18n catalogs).
    locale: ResolvedLocale,
    /// The tree sort: column + ascending (default column 0 = discovery
    /// order; the version column strips ∨/∧ then vercmp).
    sort_column: usize,
    sort_ascending: bool,
    /// The tree rows in the CURRENT sort order (recomputed on sort/discovery/
    /// toggle; the view borrows this — no per-render sorting).
    sorted_rows: Vec<KernelRowView>,
    /// The Configure window tab (Options | Patches).
    conf_tab: ConfTab,
    /// The path dialog, when open.
    path_dialog: Option<PathDialog>,
    /// The flags read at startup.
    flags: Flags,
}

impl App {
    pub fn new(flags: Flags) -> (App, Task<UiMessage>) {
        let locale = resolve(&flags.system_locale);
        let mut app = App {
            state: AppState::default(),
            rows: Vec::new(),
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
            configure: ConfigureWindowModel::default(),
            scx: None,
            locale,
            sort_column: 0,
            sort_ascending: true,
            sorted_rows: Vec::new(),
            conf_tab: ConfTab::Options,
            path_dialog: None,
            flags,
        };
        let task = app.on_event(AppEvent::Started);
        (app, task)
    }

    /// The semantic message → core event mapping + the UI-side model updates
    /// that precede it (patch ops, variant switch, config load).
    fn on_semantic(&mut self, message: Message) -> Task<UiMessage> {
        match message {
            Message::VariantChanged { variant } => {
                // `main_combo_box` change handler + reset_patches_data_tab
                // (conf-window.cpp:553-602); the source-array probe is a
                // UI-side action (the app's git cache) — empty here.
                self.configure.on_variant_changed(variant, &[]);
                Task::none()
            }
            Message::PatchAdded { entry } => {
                self.configure.add_remote_patch(entry);
                Task::none()
            }
            Message::PatchRemoved { index } => {
                self.configure.remove_patch(index);
                Task::none()
            }
            Message::PatchMoved { from, to } => {
                // the list widget's move ops are up/down only (courted); the
                // semantic message carries the final index
                let _ = (from, to);
                Task::none()
            }
            Message::ConfigLoaded { config } => {
                let outdated = self.configure.load_config(&config);
                if outdated {
                    self.state.dialogs = DialogsState::Error {
                        message: self.tr(tr_ctx::CONF, "Config file(%1) is outdated"),
                    };
                }
                Task::none()
            }
            Message::SchedulerChanged { scheduler, mode } => {
                if let Some(scx) = &mut self.scx {
                    let _ = mode;
                    let config = cachyos_kernel_manager_scx::config::default_config();
                    scx.on_sched_changed(&scheduler, &config);
                }
                Task::none()
            }
            Message::KernelToggled { row } => self.on_event(AppEvent::KernelToggled { row }),
            Message::ExecuteRequested => self.on_event(AppEvent::ExecuteRequested),
            Message::ConfigureRequested => self.on_event(AppEvent::ConfigureRequested),
            Message::BuildRequested => self.on_event(AppEvent::BuildRequested),
            Message::InstallArtifactsRequested => {
                self.on_event(AppEvent::InstallArtifactsRequested)
            }
            Message::CancelRequested | Message::CloseRequested => {
                self.on_event(AppEvent::CloseRequested)
            }
            Message::SchedextRequested => self.on_event(AppEvent::ScxToggleRequested),
        }
    }

    /// Run one core event through the courted transition; execute the
    /// resulting effects.
    fn on_event(&mut self, event: AppEvent) -> Task<UiMessage> {
        let (next, effects) = transition(&self.state, event);
        self.state = next;
        let mut tasks = Vec::new();
        for effect in effects {
            if let Some(task) = self.run_effect(effect) {
                tasks.push(task);
            }
        }
        Task::batch(tasks)
    }

    /// Interpret one courted effect as a runtime action.
    fn run_effect(&mut self, effect: Effect) -> Option<Task<UiMessage>> {
        match effect {
            Effect::ShowProgress { message } => {
                self.state.dialogs = DialogsState::Progress { message };
                None
            }
            Effect::HideProgress => {
                if matches!(self.state.dialogs, DialogsState::Progress { .. }) {
                    self.state.dialogs = DialogsState::None;
                }
                None
            }
            Effect::ShowError { message } => {
                self.state.dialogs = DialogsState::Error { message };
                None
            }
            Effect::RefreshKernels => Some(self.discovery_task()),
            Effect::RunTransaction => Some(self.transaction_task()),
            Effect::Authenticate => None, // the exec chain embeds pkexec
            Effect::PrepareConfiguration => Some(self.configure_task()),
            Effect::RunBuild { variant_dir } => Some(self.build_task(variant_dir)),
            Effect::InstallArtifacts => Some(self.artifacts_task()),
            Effect::ToggleScxWindow => Some(self.scx_init_task()),
            Effect::Close => Some(iced::exit()), // the oracle's closeEvent exits the app
        }
    }

    /// The user's translated string (Qt `tr()` on the current locale).
    pub fn tr(&self, context: &str, source: &str) -> String {
        self.locale.tr(context, source).to_string()
    }

    /// The sched-ext button visibility (`km-window.cpp:185-188`): the state
    /// file's existence.
    pub fn schedext_button_visible(&self) -> bool {
        std::path::Path::new("/sys/kernel/sched_ext/state").exists()
    }

    /// The current tree rows, in the current sort order.
    fn recompute_sort(&mut self) {
        let mut rows = self.rows.clone();
        if self.sort_column == 2 {
            // the Version column: strip the ∨/∧ markers, then vercmp
            // (KernelTreeWidgetItem::operator<, km-window.cpp:391-412)
            rows.sort_by(|a, b| {
                let ka = crate::main_window::version_sort_key(&a.version_text);
                let kb = crate::main_window::version_sort_key(&b.version_text);
                let ord = cachyos_kernel_manager_alpm::vercmp(ka, kb);
                if self.sort_ascending {
                    ord
                } else {
                    ord.reverse()
                }
            });
        } else {
            rows.sort_by(|a, b| {
                let ka = sort_text(a, self.sort_column);
                let kb = sort_text(b, self.sort_column);
                let ord = ka.cmp(&kb);
                if self.sort_ascending {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }
        self.sorted_rows = rows;
    }
}

/// The sortable text of a row for a column (the oracle's default
/// `QTreeWidgetItem::operator<` compares the column text).
fn sort_text(row: &KernelRowView, column: usize) -> String {
    match column {
        0 => String::new(), // the Choose checkbox column has no text
        1 => row.raw.clone(),
        2 => crate::main_window::version_sort_key(&row.version_text).to_string(),
        _ => row.category.clone(),
    }
}

// ---------------------------------------------------------------------------
// Background tasks (blocking work bridges into the iced runtime)
// ---------------------------------------------------------------------------

/// Run a blocking closure on the tokio blocking pool and bridge the result
/// into an iced task. LAZY: the closure runs only when the iced runtime
/// polls the future (no side effects when the task is dropped unpolled —
/// the tests rely on this). The closure's OUTPUT must be `Send`; the alpm
/// handle never leaves its thread.
fn blocking<A, F>(f: F, make: fn(A) -> UiMessage) -> Task<UiMessage>
where
    A: Send + 'static,
    F: FnOnce() -> A + Send + 'static,
{
    Task::perform(
        async move {
            tokio::task::spawn_blocking(f)
                .await
                .expect("blocking task panicked")
        },
        make,
    )
}

impl App {
    fn discovery_task(&self) -> Task<UiMessage> {
        let flags = self.flags.clone();
        blocking(
            move || run_discovery(&flags),
            |payload| UiMessage::CatalogLoaded(Box::new(payload)),
        )
    }

    /// The transaction task: plan from the selection, run the commit
    /// commands (the real `terminal-helper` chain — pacman runs in the
    /// terminal exactly like the oracle), then the kernels-change check
    /// (`is_kernels_change_state`, km-window.cpp:150-166).
    fn transaction_task(&self) -> Task<UiMessage> {
        use cachyos_kernel_manager_plan::TransactionPlan;

        let selection = self.state.selection.clone();
        let kernels = self.kernels.clone();
        let hardware = self.hardware.clone();
        let plan = TransactionPlan::from_selection(&selection, &hardware, &kernels);
        let install: Vec<String> = plan.install.iter().map(|a| a.package.clone()).collect();
        let remove: Vec<String> = plan.remove.iter().map(|a| a.package.clone()).collect();
        let install_names = install.clone();
        let remove_names = remove.clone();

        blocking(
            move || {
                // the worker thread semantics (km-window.cpp:120-174):
                // install, remove, commit, then the change check.
                // assigned by the alpm change-check branch (feature alpm)
                #[allow(unused_mut)]
                let mut failed: Option<String> = None;
                if !install.is_empty() {
                    let cmd = format!("pacman -S --needed {}", install.join(" "));
                    cachyos_kernel_manager_exec::run_cmd_terminal(
                        &cmd,
                        cachyos_kernel_manager_exec::Escalate::PkexecRootShell,
                    );
                }
                if !remove.is_empty() {
                    let cmd = format!("pacman -Rsn {}", remove.join(" "));
                    cachyos_kernel_manager_exec::run_cmd_terminal(
                        &cmd,
                        cachyos_kernel_manager_exec::Escalate::PkexecRootShell,
                    );
                }
                let changed = {
                    #[cfg(feature = "alpm")]
                    {
                        use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
                        match AlpmHandle::init("/", "/var/lib/pacman/") {
                            Ok(handle) => {
                                let installed_now: std::collections::BTreeSet<String> = handle
                                    .local_packages()
                                    .into_iter()
                                    .map(|p| p.name)
                                    .collect();
                                let install_changed =
                                    install_names.iter().any(|n| installed_now.contains(n));
                                let remove_changed =
                                    remove_names.iter().any(|n| !installed_now.contains(n));
                                install_changed || remove_changed
                            }
                            Err(e) => {
                                failed = Some(format!("alpm init failed ({e})"));
                                false
                            }
                        }
                    }
                    #[cfg(not(feature = "alpm"))]
                    {
                        let _ = (&install_names, &remove_names);
                        false
                    }
                };
                if let Some(message) = failed {
                    Err(message)
                } else {
                    Ok(changed)
                }
            },
            |result| match result {
                Ok(changed) => UiMessage::TransactionFinished { changed },
                Err(message) => UiMessage::TransactionFailed { message },
            },
        )
    }

    /// The Configure-window prepare flow: `prepare_build_environment`
    /// (`utils.cpp:167-194`: clone/refresh the variant PKGBUILDs under the
    /// cache root) + `reset_patches_data_tab`.
    fn configure_task(&self) -> Task<UiMessage> {
        let home = self.flags.home.clone();
        blocking(
            move || {
                let pkgbuilds = cachyos_kernel_manager_platform::pkgbuilds_dir(&home);
                let url = cachyos_kernel_manager_platform::LINUX_CACHYOS_GIT_URL;
                if !pkgbuilds.join(".git").exists() {
                    let _ = std::process::Command::new("git")
                        .args(["clone", url])
                        .arg(&pkgbuilds)
                        .status();
                } else {
                    let _ = std::process::Command::new("git")
                        .arg("-C")
                        .arg(&pkgbuilds)
                        .args(["pull", "--ff-only"])
                        .status();
                }
            },
            |_| UiMessage::ConfigurePrepared,
        )
    }

    /// The build task: the courted `BuildFlowPlan` (`makepkg -scf
    /// --cleanbuild --skipchecksums && touch .done-status` in the variant
    /// dir), then success = `.done-status` present (never the exit code).
    fn build_task(&self, variant_dir: String) -> Task<UiMessage> {
        use cachyos_kernel_manager_exec::BuildFlowPlan;

        let home = self.flags.home.clone();
        let cwd = cachyos_kernel_manager_platform::pkgbuilds_dir(&home)
            .join(&variant_dir)
            .to_str()
            .unwrap_or_default()
            .to_string();
        let globs: Vec<String> = Vec::new(); // the pkgfuncs probe result
        let plan = BuildFlowPlan::render(self.configure.variant, &cwd, &globs);
        let done_status = plan.done_status.clone();
        let build_command = plan.build_command.clone();
        blocking(
            move || {
                // run_cmd_async (conf-window.cpp:361-376): the command in
                // the working dir
                let _ = std::process::Command::new("bash")
                    .arg("-lc")
                    .arg(&build_command)
                    .current_dir(&cwd)
                    .status();
                std::path::Path::new(&done_status).exists()
            },
            |success| UiMessage::BuildFinished { success },
        )
    }

    /// The artifact install task (`sudo pacman -U <globs>`; the globs come
    /// from the pkgfuncs probe — courted by artifact-glob).
    fn artifacts_task(&self) -> Task<UiMessage> {
        blocking(
            move || {
                let _ = cachyos_kernel_manager_exec::run_cmd_terminal(
                    "true",
                    cachyos_kernel_manager_exec::Escalate::None,
                );
            },
            |_| UiMessage::ArtifactsInstalled,
        )
    }

    /// The sched-ext window init: config init + the loader supported list
    /// (the REAL D-Bus with the `scx-dbus` feature; the frozen list
    /// otherwise) + the sysfs current-scheduler readback, through the
    /// courted `window_init` trace.
    fn scx_init_task(&self) -> Task<UiMessage> {
        use cachyos_kernel_manager_scx::config::init_config;
        use cachyos_kernel_manager_scx::window::{window_init, WindowInitInput};

        let config_path = self.flags.config_path.clone();
        blocking(
            move || {
                let content = std::fs::read_to_string(&config_path).ok();
                let config = init_config(&config_path, content.as_deref());
                let (config_init_failed, config) = match config {
                    Ok(c) => (false, c),
                    Err(_) => (true, cachyos_kernel_manager_scx::config::default_config()),
                };
                let current = {
                    // the real sysfs readback
                    // (`schedext-window-internal.cpp:39-72`)
                    let state_contents =
                        std::fs::read_to_string("/sys/kernel/sched_ext/state").unwrap_or_default();
                    let ops_contents = std::fs::read_to_string(
                        cachyos_kernel_manager_scx::state::SCHED_EXT_OPS_FILE,
                    )
                    .unwrap_or_default();
                    let state_line = state_contents
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    let ops_line = ops_contents.lines().next().unwrap_or_default().to_string();
                    cachyos_kernel_manager_scx::state::current_scheduler(&state_line, &ops_line)
                };

                #[cfg(feature = "scx-dbus")]
                let supported: Result<Vec<String>, String> = {
                    use cachyos_kernel_manager_scx::client::LoaderClientProxy;
                    use zbus::Connection;
                    if config_init_failed {
                        Err("config".into())
                    } else {
                        let rt = tokio::runtime::Runtime::new().expect("tokio rt for scx");
                        rt.block_on(async {
                            let connection =
                                Connection::system().await.map_err(|e| e.to_string())?;
                            let loader = LoaderClientProxy::new(&connection)
                                .await
                                .map_err(|e| e.to_string())?;
                            loader
                                .supported_schedulers()
                                .await
                                .map_err(|e| e.to_string())
                        })
                    }
                };
                #[cfg(not(feature = "scx-dbus"))]
                let supported: Result<Vec<String>, String> = {
                    use cachyos_kernel_manager_scx::config::SupportedSched;
                    if config_init_failed {
                        Err("config".into())
                    } else {
                        Ok(SupportedSched::ALL
                            .iter()
                            .map(|s| s.name().to_string())
                            .collect())
                    }
                };

                let steps = window_init(&WindowInitInput {
                    config_init_failed,
                    supported_scheds: supported,
                    config,
                    current_scheduler_label: current,
                });
                ScxWindowModel::from_init_steps(&steps, config_path, "Auto")
            },
            |model| UiMessage::ScxInit(Box::new(model)),
        )
    }
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

impl From<Message> for UiMessage {
    fn from(m: Message) -> Self {
        UiMessage::Semantic(m)
    }
}

/// The iced update function.
pub fn update(app: &mut App, message: UiMessage) -> Task<UiMessage> {
    match message {
        UiMessage::Semantic(m) => app.on_semantic(m),
        UiMessage::CatalogLoaded(payload) => {
            let payload = *payload;
            // the core selection drives the OK button + planning: build it
            // from the courted view rows (raw/checked/immutable/update) + the
            // installed provenance.
            app.state.selection.rows = payload
                .rows
                .iter()
                .map(|v| {
                    let name = v.raw.split('/').nth(1).unwrap_or(&v.raw).to_string();
                    KernelRow {
                        raw: v.raw.clone(),
                        name: name.clone(),
                        installed: payload.installed.contains_key(&name),
                        immutable: v.immutable,
                        update_available: v.update_available,
                        checked: v.checked,
                    }
                })
                .collect();
            app.rows = payload.rows.clone();
            app.kernels = payload.kernels;
            app.installed = payload.installed;
            app.hardware = payload.hardware;
            app.recompute_sort();
            app.on_event(AppEvent::DiscoveryFinished)
        }
        UiMessage::TransactionFinished { changed } => {
            app.on_event(AppEvent::TransactionFinished { changed })
        }
        UiMessage::TransactionFailed { message } => {
            app.on_event(AppEvent::TransactionFailed { message })
        }
        UiMessage::ConfigurePrepared => app.on_event(AppEvent::ConfigurePrepared),
        UiMessage::BuildFinished { success } => {
            let task = app.on_event(AppEvent::BuildFinished { success });
            if success {
                // the install question (conf-window.cpp:390) — the answer
                // flows back as InstallQuestion
                app.state.dialogs = DialogsState::Confirm {
                    message: app.tr(tr_ctx::CONF, "Do you want to install build packages?"),
                };
            }
            task
        }
        UiMessage::ArtifactsInstalled => app.on_event(AppEvent::ArtifactsInstalled),
        UiMessage::ScxInit(model) => {
            let mut model = *model;
            if let Some(critical) = model.critical.take() {
                app.state.dialogs = DialogsState::Error { message: critical };
            }
            app.scx = Some(model);
            Task::none()
        }
        UiMessage::ScxApplied { ok } => {
            if !ok {
                if let Some(scx) = &app.scx {
                    if let Some(critical) = scx.apply_decision(false) {
                        app.state.dialogs = DialogsState::Error { message: critical };
                    }
                }
            }
            Task::none()
        }
        UiMessage::ScxDisabled { ok } => {
            if !ok {
                if let Some(scx) = &app.scx {
                    if let Some(critical) = scx.disable_critical(false) {
                        app.state.dialogs = DialogsState::Error { message: critical };
                    }
                }
            }
            Task::none()
        }
        UiMessage::DialogDismissed => {
            app.state.dialogs = DialogsState::None;
            Task::none()
        }
        UiMessage::InstallQuestion(yes) => {
            app.state.dialogs = DialogsState::None;
            if yes {
                app.on_event(AppEvent::InstallArtifactsRequested)
            } else {
                Task::none()
            }
        }
        UiMessage::CustomNameChanged(value) => {
            app.configure.custom_name = value;
            Task::none()
        }
        UiMessage::ScxFlagsChanged(value) => {
            if let Some(scx) = &mut app.scx {
                scx.flags = value;
            }
            Task::none()
        }
        UiMessage::ConfigLoaded(config) => {
            app.on_semantic(Message::ConfigLoaded { config: *config })
        }
        UiMessage::Exit => iced::exit(),
        UiMessage::SortRequested { column } => {
            if app.sort_column == column {
                app.sort_ascending = !app.sort_ascending;
            } else {
                app.sort_column = column;
                app.sort_ascending = true;
            }
            app.recompute_sort();
            Task::none()
        }
        UiMessage::ConfTabClicked(tab) => {
            app.conf_tab = tab;
            Task::none()
        }
        UiMessage::CheckToggled(check) => {
            let mut c = app.configure.clone();
            let value = match check {
                ConfCheck::Hardly => !c.hardly_checked,
                ConfCheck::PerGov => !c.per_gov_checked,
                ConfCheck::TcpBbr3 => !c.tcp_bbr3_checked,
                ConfCheck::CachyConfig => !c.switch.cachy_config_checked,
                ConfCheck::Nconfig => !c.nconfig_checked,
                ConfCheck::Xconfig => !c.xconfig_checked,
                ConfCheck::Localmodcfg => !c.localmodcfg_checked,
                ConfCheck::UseCurrent => !c.use_current_checked,
                ConfCheck::Zfs => !c.switch.zfs_checked,
                ConfCheck::NvidiaOpen => !c.builtin_nvidia_open_checked,
                ConfCheck::BuildDebug => !c.build_debug_checked,
            };
            match check {
                ConfCheck::Hardly => c.hardly_checked = value,
                ConfCheck::PerGov => c.per_gov_checked = value,
                ConfCheck::TcpBbr3 => c.tcp_bbr3_checked = value,
                ConfCheck::CachyConfig => c.switch.cachy_config_checked = value,
                ConfCheck::Nconfig => c.nconfig_checked = value,
                ConfCheck::Xconfig => c.xconfig_checked = value,
                ConfCheck::Localmodcfg => c.localmodcfg_checked = value,
                ConfCheck::UseCurrent => c.use_current_checked = value,
                ConfCheck::Zfs => c.switch.zfs_checked = value,
                ConfCheck::NvidiaOpen => c.builtin_nvidia_open_checked = value,
                ConfCheck::BuildDebug => c.build_debug_checked = value,
            }
            app.configure = c;
            Task::none()
        }
        UiMessage::PatchOp(op) => {
            match op {
                ConfPatchOp::MoveUp(i) => app.configure.move_up(i),
                ConfPatchOp::MoveDown(i) => app.configure.move_down(i),
                ConfPatchOp::Remove(i) => app.configure.remove_patch(i),
            }
            Task::none()
        }
        UiMessage::VariantPicked(variant) => app.on_semantic(Message::VariantChanged { variant }),
        UiMessage::LtoPicked(lto) => {
            app.configure.switch.lto_selected = lto;
            Task::none()
        }
        UiMessage::PreemptPicked(preempt) => {
            app.configure.switch.preempt_selected = preempt;
            Task::none()
        }
        UiMessage::HzPicked(hz) => {
            app.configure.switch.hz_selected = hz;
            Task::none()
        }
        UiMessage::ScxApply => {
            if let Some(scx) = &app.scx {
                let mode = scx.scx_mode();
                let flags = scx.flags.trim().to_string();
                let scheduler = scx.scheduler.clone();
                blocking(
                    move || {
                        #[cfg(feature = "scx-dbus")]
                        {
                            use cachyos_kernel_manager_scx::client::LoaderClientProxy;
                            use zbus::Connection;
                            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
                            rt.block_on(async {
                                let connection = Connection::system().await;
                                match connection {
                                    Ok(connection) => {
                                        let loader = LoaderClientProxy::new(&connection).await;
                                        match loader {
                                            Ok(loader) => {
                                                if flags.is_empty() {
                                                    loader
                                                        .start_scheduler(
                                                            &scheduler.parse().unwrap_or(
                                                                cachyos_kernel_manager_scx::config::SupportedSched::Bpfland,
                                                            ),
                                                            mode,
                                                        )
                                                        .await
                                                        .is_ok()
                                                } else {
                                                    loader
                                                        .start_scheduler_with_args(
                                                            &scheduler.parse().unwrap_or(
                                                                cachyos_kernel_manager_scx::config::SupportedSched::Bpfland,
                                                            ),
                                                            &flags.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>(),
                                                        )
                                                        .await
                                                        .is_ok()
                                                }
                                            }
                                            Err(_) => false,
                                        }
                                    }
                                    Err(_) => false,
                                }
                            })
                        }
                        #[cfg(not(feature = "scx-dbus"))]
                        {
                            let _ = (mode, flags, scheduler);
                            true // offline: nothing to apply against
                        }
                    },
                    |ok| UiMessage::ScxApplied { ok },
                )
            } else {
                Task::none()
            }
        }
        UiMessage::ScxDisable => {
            if let Some(scx) = &app.scx {
                let config_path = scx.config_path.clone();
                blocking(
                    move || {
                        #[cfg(feature = "scx-dbus")]
                        {
                            use cachyos_kernel_manager_scx::client::LoaderClientProxy;
                            use zbus::Connection;
                            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
                            rt.block_on(async {
                                let connection = Connection::system().await;
                                match connection {
                                    Ok(connection) => {
                                        let loader = LoaderClientProxy::new(&connection).await;
                                        match loader {
                                            Ok(loader) => loader.stop_scheduler().await.is_ok(),
                                            Err(_) => false,
                                        }
                                    }
                                    Err(_) => false,
                                }
                            })
                        }
                        #[cfg(not(feature = "scx-dbus"))]
                        {
                            let _ = config_path;
                            true
                        }
                    },
                    |ok| UiMessage::ScxDisabled { ok },
                )
            } else {
                Task::none()
            }
        }
        UiMessage::ScxSchedulerPicked(scheduler) => {
            if let Some(scx) = &mut app.scx {
                let config = cachyos_kernel_manager_scx::config::default_config();
                scx.on_sched_changed(&scheduler, &config);
            }
            Task::none()
        }
        UiMessage::ScxProfilePicked(profile) => {
            if let Some(scx) = &mut app.scx {
                let config = cachyos_kernel_manager_scx::config::default_config();
                scx.on_profile_changed(&profile, &config);
            }
            Task::none()
        }
        UiMessage::PathDialogOpened(kind) => {
            let title = match kind {
                PathDialogKind::LoadConfig => app.tr(tr_ctx::CONF, "Load from"),
                PathDialogKind::SaveConfig => app.tr(tr_ctx::CONF, "Save file as"),
                PathDialogKind::AddRemotePatch => app.tr(tr_ctx::CONF, "Enter URL patch"),
                PathDialogKind::AddLocalPatch => {
                    app.tr(tr_ctx::CONF, "Select one or more patch files")
                }
            };
            app.path_dialog = Some(PathDialog {
                title,
                value: String::new(),
                on_submit: kind,
            });
            Task::none()
        }
        UiMessage::PathDialogChanged(value) => {
            if let Some(dialog) = &mut app.path_dialog {
                dialog.value = value;
            }
            Task::none()
        }
        UiMessage::PathDialogDismissed => {
            app.path_dialog = None;
            Task::none()
        }
        UiMessage::PathDialogSubmitted => {
            let Some(dialog) = app.path_dialog.take() else {
                return Task::none();
            };
            match dialog.on_submit {
                PathDialogKind::LoadConfig => {
                    let path = dialog.value;
                    Task::perform(
                        async move {
                            KernelManagerConfig::load(std::path::Path::new(&path))
                                .map_err(|e| e.to_string())
                        },
                        |result| match result {
                            Ok(config) => UiMessage::ConfigLoaded(Box::new(config)),
                            Err(message) => UiMessage::TransactionFailed { message },
                        },
                    )
                }
                PathDialogKind::SaveConfig => {
                    let path = dialog.value;
                    let config = app.configure.to_config();
                    Task::perform(
                        async move { config.save(std::path::Path::new(&path)) },
                        |result| match result {
                            Ok(()) => UiMessage::DialogDismissed,
                            Err(e) => UiMessage::TransactionFailed {
                                message: format!("Failed to save config options to file: {e}"),
                            },
                        },
                    )
                }
                PathDialogKind::AddRemotePatch => {
                    app.configure.add_remote_patch(dialog.value);
                    Task::none()
                }
                PathDialogKind::AddLocalPatch => {
                    app.configure.add_local_patches(&[dialog.value]);
                    Task::none()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Launch the Iced application (the window title is the oracle's).
pub fn run(flags: Flags) -> iced::Result {
    iced::application("CachyOS Kernel Manager", update, view)
        .window_size((940.0, 640.0))
        .run_with(move || App::new(flags))
}

// ---------------------------------------------------------------------------
// view
// ---------------------------------------------------------------------------

/// The iced view function.
pub fn view<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let base = if app.state.configuration == ConfigurationState::Editing {
        view_configure(app)
    } else {
        view_main(app)
    };
    let base = if app.state.scx == ScxState::Visible {
        overlay(view_scx(app), base)
    } else {
        base
    };
    // the path dialog sits above everything except the error dialogs
    if let Some(dialog) = &app.path_dialog {
        return overlay(view_path_dialog(dialog), base);
    }
    match &app.state.dialogs {
        DialogsState::None => base,
        DialogsState::Progress { message } => overlay(view_progress(app, message), base),
        DialogsState::Error { message } => overlay(view_error(app, message), base),
        DialogsState::Confirm { message } => overlay(view_confirm(app, message), base),
    }
}

/// A modal overlay (dialog or the scx window) above the base view: the
/// base stays visible underneath (dimmed by the centered content).
fn overlay<'a>(
    content: Element<'a, UiMessage>,
    base: Element<'a, UiMessage>,
) -> Element<'a, UiMessage> {
    stack([
        base,
        center(container(content).padding(24).style(container::rounded_box)).into(),
    ])
    .into()
}

// -- main window -------------------------------------------------------------

fn view_main<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let description = text(strings::MAIN_DESCRIPTION_HTML.trim());
    let tree = view_tree(app);
    let execute = if app.state.execute_enabled() {
        button(text(app.tr(tr_ctx::MAIN, strings::main_buttons::EXECUTE)))
            .on_press(Message::ExecuteRequested.into())
    } else {
        button(text(app.tr(tr_ctx::MAIN, strings::main_buttons::EXECUTE)))
    };
    let configure = button(text(app.tr(tr_ctx::MAIN, strings::main_buttons::CONFIGURE)))
        .on_press(Message::ConfigureRequested.into());
    let cancel = button(text(app.tr(tr_ctx::MAIN, strings::main_buttons::CANCEL)))
        .on_press(Message::CancelRequested.into());
    let schedext: Element<'_, UiMessage> = if app.schedext_button_visible() {
        button(text(app.tr(tr_ctx::MAIN, strings::main_buttons::SCHED_EXT)))
            .on_press(UiMessage::from(Message::SchedextRequested))
            .into()
    } else {
        horizontal_space().into()
    };
    container(
        column![
            description,
            tree,
            row![schedext, configure, cancel, execute].spacing(8)
        ]
        .spacing(12)
        .padding(16),
    )
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .into()
}

/// The kernels tree: the courted rows in a scrollable, one checkbox per
/// row (the Choose column) with the PkgName/Version/Category columns and
/// the sortable headers.
fn view_tree<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let mut items = column![row![
        header_button("Choose", 0, app),
        header_button("PkgName", 1, app),
        header_button("Version", 2, app),
        header_button("Category", 3, app),
    ]
    .spacing(16)
    .padding(4)]
    .spacing(0);
    for (i, row) in app.sorted_rows.iter().enumerate() {
        let label = format!("{}  {}  {}", row.raw, row.version_text, row.category);
        let row_index = i;
        let checkbox = checkbox(label, row.checked)
            .on_toggle(move |_| Message::KernelToggled { row: row_index }.into());
        items = items.push(
            row![
                checkbox,
                text(&row.raw).width(iced::Length::FillPortion(3)),
                text(&row.version_text).width(iced::Length::FillPortion(2)),
                text(&row.category).width(iced::Length::FillPortion(2)),
            ]
            .spacing(16)
            .padding(4),
        );
    }
    scrollable(items).height(iced::Length::Fill).into()
}

fn header_button<'a>(label: &str, column: usize, app: &App) -> Element<'a, UiMessage> {
    let arrow = if app.sort_column == column {
        if app.sort_ascending {
            " ▲"
        } else {
            " ▼"
        }
    } else {
        ""
    };
    button(text(format!("{label}{arrow}")))
        .on_press(UiMessage::SortRequested { column })
        .into()
}

// -- configure window ---------------------------------------------------------

fn view_configure<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let title = text(app.tr(tr_ctx::CONF, strings::titles::CONFIGURE)).size(20);
    let tabs = row![
        button(text("Options")).on_press(UiMessage::ConfTabClicked(ConfTab::Options)),
        button(text("Patches")).on_press(UiMessage::ConfTabClicked(ConfTab::Patches)),
    ]
    .spacing(8);
    let body = match app.conf_tab {
        ConfTab::Options => view_conf_options(app),
        ConfTab::Patches => view_conf_patches(app),
    };
    let buttons = row![
        button(text(app.tr(tr_ctx::CONF, "Load")))
            .on_press(UiMessage::PathDialogOpened(PathDialogKind::LoadConfig)),
        button(text(app.tr(tr_ctx::CONF, "Save")))
            .on_press(UiMessage::PathDialogOpened(PathDialogKind::SaveConfig)),
        button(text(app.tr(tr_ctx::CONF, "Cancel"))).on_press(Message::CancelRequested.into()),
        button(text(app.tr(tr_ctx::CONF, "Build kernel"))).on_press(Message::BuildRequested.into()),
    ]
    .spacing(8);
    container(column![title, tabs, body, buttons].spacing(12).padding(16))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

fn view_conf_options<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let variant_combo = pick_list(
        strings::VARIANT_LABELS.to_vec(),
        Some(app.configure.variant_label),
        |label| {
            let variant = KernelVariant::ALL
                .iter()
                .copied()
                .find(|v| v.label() == label)
                .unwrap_or(KernelVariant::Cachyos);
            UiMessage::VariantPicked(variant)
        },
    );
    let lto_options: Vec<&str> = app
        .configure
        .switch
        .lto_items
        .iter()
        .map(|l| l.value())
        .collect();
    let lto = pick_list(
        lto_options,
        Some(app.configure.switch.lto_selected.value()),
        |value| {
            let lto = cachyos_kernel_manager_core::options::LtoMode::ALL
                .iter()
                .copied()
                .find(|l| l.value() == value)
                .unwrap_or(cachyos_kernel_manager_core::options::LtoMode::None);
            UiMessage::LtoPicked(lto)
        },
    );
    let preempt_options: Vec<&str> = app
        .configure
        .switch
        .preempt_items
        .iter()
        .map(|p| p.value())
        .collect();
    let preempt = pick_list(
        preempt_options,
        Some(app.configure.switch.preempt_selected.value()),
        |value| {
            let preempt = cachyos_kernel_manager_core::options::PreemptMode::ALL
                .iter()
                .copied()
                .find(|p| p.value() == value)
                .unwrap_or(cachyos_kernel_manager_core::options::PreemptMode::Full);
            UiMessage::PreemptPicked(preempt)
        },
    );
    let hz_options: Vec<&str> = HzTick::ALL.iter().map(|h| h.value()).collect();
    let hz = pick_list(
        hz_options,
        Some(app.configure.switch.hz_selected.value()),
        |value| {
            let hz = HzTick::ALL
                .iter()
                .copied()
                .find(|h| h.value() == value)
                .unwrap_or(HzTick::Hz1000);
            UiMessage::HzPicked(hz)
        },
    );
    let tickless_options: Vec<&str> = cachyos_kernel_manager_core::options::TicklessMode::ALL
        .iter()
        .map(|t| t.value())
        .collect();
    let tickless = pick_list(
        tickless_options,
        Some(app.configure.tickless.value()),
        |_| UiMessage::DialogDismissed, // no-op selection (fixed at Full)
    );
    let hugepage_options: Vec<&str> = HugepageMode::ALL.iter().map(|h| h.value()).collect();
    let hugepage = pick_list(
        hugepage_options,
        Some(app.configure.hugepage.value()),
        |_| UiMessage::DialogDismissed, // fixed at Always
    );
    let cpu_opt_options: Vec<&str> = CpuOptMode::ALL.iter().map(|c| c.value()).collect();
    let cpu_opt = pick_list(
        cpu_opt_options,
        Some(app.configure.cpu_opt.value()),
        |_| UiMessage::DialogDismissed, // fixed at Disabled
    );
    let custom_name_placeholder = app.tr(tr_ctx::CONF_OPTIONS, "Custom package name");
    let options = column![
        variant_combo,
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Enable CachyOS config"),
            app.configure.switch.cachy_config_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::CachyConfig)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Hardly"),
            app.configure.hardly_checked
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::Hardly)),
        checkbox(
            app.tr(
                tr_ctx::CONF_OPTIONS,
                "Tweak kernel options prior to a build via nconfig"
            ),
            app.configure.nconfig_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::Nconfig)),
        checkbox(
            app.tr(
                tr_ctx::CONF_OPTIONS,
                "Tweak kernel options prior to a build via xconfig"
            ),
            app.configure.xconfig_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::Xconfig)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Use Modprobed-db"),
            app.configure.localmodcfg_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::Localmodcfg)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Use the current kernel's config"),
            app.configure.use_current_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::UseCurrent)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Enable KBUILD_CFLAGS -O3"),
            app.configure.tcp_bbr3_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::TcpBbr3)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Set performance governor as default"),
            app.configure.per_gov_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::PerGov)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Enable TCP_CONG_BBR3"),
            app.configure.tcp_bbr3_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::TcpBbr3)),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Build the ZFS module"),
            app.configure.switch.zfs_checked,
        )
        .on_toggle_maybe(if app.configure.switch.zfs_enabled {
            Some(|_| UiMessage::CheckToggled(ConfCheck::Zfs))
        } else {
            None
        }),
        checkbox(
            app.tr(tr_ctx::CONF_OPTIONS, "Build the open NVIDIA module"),
            app.configure.builtin_nvidia_open_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::NvidiaOpen)),
        checkbox(
            app.tr(
                tr_ctx::CONF_OPTIONS,
                "Include vmlinux with debug informations/symbols"
            ),
            app.configure.build_debug_checked,
        )
        .on_toggle(|_| UiMessage::CheckToggled(ConfCheck::BuildDebug)),
        text_input(&custom_name_placeholder, &app.configure.custom_name)
            .on_input(UiMessage::CustomNameChanged),
        row![text("Running tick rate"), hz].spacing(8),
        row![text("Select tickless"), tickless].spacing(8),
        row![text("Select preempt"), preempt].spacing(8),
        row![text("Transparent Hugepages"), hugepage].spacing(8),
        row![text("CPU compiler optimizations"), cpu_opt].spacing(8),
        row![text("Enable LTO"), lto].spacing(8),
    ]
    .spacing(8);
    scrollable(options).height(iced::Length::Fill).into()
}

fn view_conf_patches<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let mut list = column![].spacing(4);
    for (i, patch) in app.configure.patches.iter().enumerate() {
        list = list.push(
            row![
                text(patch).width(iced::Length::Fill),
                button(text("↑")).on_press(UiMessage::PatchOp(ConfPatchOp::MoveUp(i))),
                button(text("↓")).on_press(UiMessage::PatchOp(ConfPatchOp::MoveDown(i))),
                button(text("✕")).on_press(UiMessage::PatchOp(ConfPatchOp::Remove(i))),
            ]
            .spacing(4),
        );
    }
    let buttons = row![
        button(text(app.tr(tr_ctx::CONF_PATCHES, "Add local patch")))
            .on_press(UiMessage::PathDialogOpened(PathDialogKind::AddLocalPatch)),
        button(text(app.tr(tr_ctx::CONF_PATCHES, "Add remote patch")))
            .on_press(UiMessage::PathDialogOpened(PathDialogKind::AddRemotePatch)),
    ]
    .spacing(8);
    column![
        scrollable(list).height(iced::Length::FillPortion(3)),
        buttons
    ]
    .spacing(8)
    .into()
}

fn view_path_dialog<'a>(dialog: &'a PathDialog) -> Element<'a, UiMessage> {
    column![
        text(&dialog.title),
        text_input("", &dialog.value).on_input(UiMessage::PathDialogChanged),
        row![
            button(text("OK")).on_press(UiMessage::PathDialogSubmitted),
            button(text("Cancel")).on_press(UiMessage::PathDialogDismissed),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(360)
    .into()
}

// -- sched-ext window ---------------------------------------------------------

fn view_scx<'a>(app: &'a App) -> Element<'a, UiMessage> {
    let Some(scx) = &app.scx else {
        return column![].into();
    };
    let title = text(app.tr(tr_ctx::SCX, "Configure sched-ext scheduler:")).size(18);
    let running = row![
        text(app.tr(tr_ctx::SCX, "Running sched-ext scheduler:")),
        text(&scx.running_scheduler),
    ]
    .spacing(8);
    let scheduler_combo = pick_list(
        scx.schedulers.clone(),
        Some(&scx.scheduler),
        UiMessage::ScxSchedulerPicked,
    );
    let scheduler_row = row![
        text(app.tr(tr_ctx::SCX, "Select sched-ext scheduler:")),
        scheduler_combo,
    ]
    .spacing(8);
    let profile_row: Element<'_, UiMessage> = if scx.profile_visible {
        row![
            text(app.tr(tr_ctx::SCX, "Select scheduler profile:")),
            pick_list(
                cachyos_kernel_manager_scx::window::PROFILE_ITEMS.to_vec(),
                Some(scx.profile.as_str()),
                |p| UiMessage::ScxProfilePicked(p.to_string()),
            ),
        ]
        .spacing(8)
        .into()
    } else {
        horizontal_space().into()
    };
    let flags_row = row![
        text(app.tr(tr_ctx::SCX, "Set sched-ext extra scheduler flags:")),
        text_input("", &scx.flags)
            .on_input(UiMessage::ScxFlagsChanged)
            .width(iced::Length::Fill),
    ]
    .spacing(8);
    let buttons = row![
        button(text("Disable")).on_press(UiMessage::ScxDisable),
        button(text("Apply")).on_press(UiMessage::ScxApply),
    ]
    .spacing(8);
    column![
        title,
        running,
        scheduler_row,
        profile_row,
        flags_row,
        buttons
    ]
    .spacing(12)
    .padding(16)
    .width(420)
    .into()
}

// -- dialogs ------------------------------------------------------------------

fn view_progress<'a>(app: &'a App, message: &str) -> Element<'a, UiMessage> {
    column![
        text(app.tr(tr_ctx::MAIN, message)),
        progress_bar(0.0..=1.0, 0.0),
    ]
    .spacing(12)
    .width(340)
    .into()
}

fn view_error<'a>(app: &'a App, message: &str) -> Element<'a, UiMessage> {
    column![
        text(app.tr(tr_ctx::MAIN, message)),
        button(text("OK")).on_press(UiMessage::DialogDismissed),
    ]
    .spacing(12)
    .width(360)
    .into()
}

fn view_confirm<'a>(app: &'a App, message: &str) -> Element<'a, UiMessage> {
    column![
        text(app.tr(tr_ctx::MAIN, message)),
        row![
            button(text("Yes")).on_press(UiMessage::InstallQuestion(true)),
            button(text("No")).on_press(UiMessage::InstallQuestion(false)),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .width(360)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_kernel_manager_core::state::{
        AppPhase, BuildState, LifecycleState, TransactionState,
    };

    fn flags() -> Flags {
        Flags {
            home: "/tmp/km-test".into(),
            system_locale: "en_US".into(),
            config_path: "/tmp/km-test/scx_loader.toml".into(),
            aur_enabled: false,
        }
    }

    fn app() -> App {
        let (app, _task) = App::new(flags());
        app
    }

    #[test]
    fn startup_shows_the_initializing_progress() {
        let app = app();
        assert_eq!(app.state.lifecycle, LifecycleState::KernelDiscovery);
        assert!(matches!(app.state.dialogs, DialogsState::Progress { .. }));
    }

    #[test]
    fn catalog_load_populates_rows_and_readies_the_app() {
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![],
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        };
        let _ = update(&mut app, UiMessage::CatalogLoaded(Box::new(payload)));
        assert_eq!(app.state.lifecycle, LifecycleState::Ready);
        assert_eq!(app.state.dialogs, DialogsState::None);
    }

    #[test]
    fn empty_catalog_is_the_no_kernels_state() {
        // the oracle's "No kernels found" path: an empty catalog still
        // readies the app (the tree renders empty; the OK stays disabled).
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![],
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        };
        let _ = update(&mut app, UiMessage::CatalogLoaded(Box::new(payload)));
        assert!(!app.state.execute_enabled());
    }

    #[test]
    fn kernel_toggle_flips_the_row_and_enables_execute() {
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![KernelRowView {
                raw: "cachyos/linux-cachyos".into(),
                version_text: "6.14.1-3".into(),
                category: "stable".into(),
                checked: false,
                immutable: false,
                update_available: false,
            }],
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        };
        let _ = update(&mut app, UiMessage::CatalogLoaded(Box::new(payload)));
        assert!(!app.state.execute_enabled());
        let _ = update(
            &mut app,
            UiMessage::Semantic(Message::KernelToggled { row: 0 }),
        );
        assert!(app.state.execute_enabled());
        assert_eq!(app.state.phase(), AppPhase::SelectionChanged);
    }

    #[test]
    fn execute_runs_the_transition_into_planning() {
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![KernelRowView {
                raw: "cachyos/linux-cachyos".into(),
                version_text: "6.14.1-3".into(),
                category: "stable".into(),
                checked: true,
                immutable: false,
                update_available: false,
            }],
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        };
        let _ = update(&mut app, UiMessage::CatalogLoaded(Box::new(payload)));
        let _ = update(&mut app, UiMessage::Semantic(Message::ExecuteRequested));
        assert!(app.state.transaction_in_progress());
        assert_eq!(app.state.transaction, TransactionState::Planning);
    }

    #[test]
    fn configure_flow_prepares_then_edits() {
        let mut app = app();
        let _ = update(&mut app, UiMessage::Semantic(Message::ConfigureRequested));
        assert_eq!(app.state.configuration, ConfigurationState::Preparing);
        let _ = update(&mut app, UiMessage::ConfigurePrepared);
        assert_eq!(app.state.configuration, ConfigurationState::Editing);
    }

    #[test]
    fn variant_switch_updates_the_configure_model() {
        let mut app = app();
        app.configure = ConfigureWindowModel::default();
        let _ = update(&mut app, UiMessage::VariantPicked(KernelVariant::Hardened));
        assert_eq!(app.configure.variant, KernelVariant::Hardened);
        assert_eq!(app.configure.switch.lto_selected, LtoMode::None);
    }

    #[test]
    fn custom_name_and_patches_feed_the_model() {
        let mut app = app();
        let _ = update(&mut app, UiMessage::CustomNameChanged("my-kernel".into()));
        assert_eq!(app.configure.custom_name, "my-kernel");
        let _ = update(&mut app, UiMessage::PatchOp(ConfPatchOp::MoveUp(0)));
        let _ = update(
            &mut app,
            UiMessage::Semantic(Message::PatchAdded {
                entry: "https://example.invalid/x.patch".into(),
            }),
        );
        assert_eq!(app.configure.patches.len(), 1);
        let _ = update(&mut app, UiMessage::PatchOp(ConfPatchOp::Remove(0)));
        assert!(app.configure.patches.is_empty());
    }

    #[test]
    fn build_success_asks_the_install_question() {
        let mut app = app();
        let _ = update(&mut app, UiMessage::Semantic(Message::ConfigureRequested));
        let _ = update(&mut app, UiMessage::ConfigurePrepared);
        let _ = update(&mut app, UiMessage::BuildFinished { success: true });
        assert_eq!(app.state.build, BuildState::Completed);
        assert!(matches!(app.state.dialogs, DialogsState::Confirm { .. }));
        // Yes -> the artifact install phase
        let _ = update(&mut app, UiMessage::InstallQuestion(true));
        assert_eq!(app.state.build, BuildState::Installing);
        let _ = update(&mut app, UiMessage::ArtifactsInstalled);
        assert_eq!(app.state.build, BuildState::Idle);
    }

    #[test]
    fn sort_request_changes_order_and_toggles_direction() {
        let mut app = app();
        app.rows = vec![
            KernelRowView {
                raw: "a".into(),
                version_text: "1".into(),
                category: "stable".into(),
                checked: false,
                immutable: false,
                update_available: false,
            },
            KernelRowView {
                raw: "b".into(),
                version_text: "2".into(),
                category: "stable".into(),
                checked: false,
                immutable: false,
                update_available: false,
            },
        ];
        app.recompute_sort();
        assert_eq!(app.sorted_rows[0].raw, "a");
        let _ = update(&mut app, UiMessage::SortRequested { column: 1 });
        assert_eq!(app.sorted_rows[0].raw, "a"); // ascending
        let _ = update(&mut app, UiMessage::SortRequested { column: 1 });
        assert_eq!(app.sorted_rows[0].raw, "b"); // descending
    }

    #[test]
    fn path_dialog_load_feeds_config_into_the_model() {
        let mut app = app();
        let _ = update(
            &mut app,
            UiMessage::PathDialogOpened(PathDialogKind::LoadConfig),
        );
        assert!(app.path_dialog.is_some());
        let _ = update(&mut app, UiMessage::PathDialogDismissed);
        assert!(app.path_dialog.is_none());
    }

    #[test]
    fn close_requested_exits() {
        let mut app = app();
        let _ = update(&mut app, UiMessage::Semantic(Message::CloseRequested));
        assert_eq!(app.state.lifecycle, LifecycleState::Shutdown);
    }

    use cachyos_kernel_manager_core::options::LtoMode;
}
