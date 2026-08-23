//! The Slint application — the Phase 8 rendering layer (the Slint port).
//!
//! Layering discipline (docs/ARCHITECTURE.md): this module ONLY translates
//! Slint callbacks into the courted semantic substrate (core `AppState`
//! transitions, the plan/exec/build/config/scx models) and renders it. No
//! domain semantics live here; a UI bug must be attributable to this layer,
//! never to the models.
//!
//! Window strategy: the oracle has three native windows (Main/Configure/
//! SchedExt). The candidate renders them as separate Slint windows, with the
//! dialogs (progress/error/confirm) as separate windows too. The *semantics*
//! are courted; the window choreography is a rendering choice. The Configure
//! + SchedExt windows are ported; the main window shows no placeholder.

use crate::configure_window::ConfigureWindowModel;
use crate::i18n::{resolve, ResolvedLocale};
use crate::main_window::rows;
use crate::scx_window::ScxWindowModel;
use crate::strings;
use crate::strings::standard_buttons::StandardButton;
use crate::{KernelRowView, Message};
use cachyos_kernel_manager_config::KernelManagerConfig;
use cachyos_kernel_manager_core::discovery::DiscoveredKernel;
use cachyos_kernel_manager_core::options::{
    CpuOptMode, HugepageMode, HzTick, KernelVariant, LtoMode, PreemptMode, TicklessMode,
};
use cachyos_kernel_manager_core::selection::KernelRow;
use cachyos_kernel_manager_core::state::{
    transition, AppEvent, AppState, ConfigurationState, DialogsState, Effect, ScxState,
    TransactionState,
};
use cachyos_kernel_manager_plan::HardwareProfile;
use slint::ModelRc;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// The Slint-generated glue. `include_modules!()` includes only ONE generated
// file (each `slint_build::compile` overwrites `SLINT_INCLUDE_GENERATED`),
// so each window file is included into its own submodule and re-exported
// (the generated files collide on private const names if merged).
#[allow(clippy::all)]
mod slint_main_window {
    include!(concat!(env!("OUT_DIR"), "/main_window.rs"));
}
#[allow(clippy::all)]
mod slint_configure_window {
    include!(concat!(env!("OUT_DIR"), "/configure_window.rs"));
}
#[allow(clippy::all)]
mod slint_scx_window {
    include!(concat!(env!("OUT_DIR"), "/scx_window.rs"));
}
pub use slint_configure_window::{ConfCheckRow, ConfigureWindow};
pub use slint_main_window::{MainWindow, TreeRow};
pub use slint_scx_window::SchedExtWindow;
// the generated glue imports ComponentHandle with an underscore alias INSIDE
// each submodule; re-export it here so `show/hide/as_weak/run/window` work
// on the re-exported component types.
pub use slint::ComponentHandle;

// ---------------------------------------------------------------------------
// Verbose logging
// ---------------------------------------------------------------------------

/// Whether verbose mode is on (`KM_VERBOSE=1` env or `--verbose` arg). The
/// probes + background tasks run on worker threads, so this is process-global.
static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The process start instant (the `[km]` trace timestamps are elapsed since
/// start — `Instant::now().elapsed()` on a fresh instant would always be ~0).
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// The optional trace log file (`KM_LOG_FILE=/path`): the `[km]` lines AND
/// the important diagnostics (probe timeouts, panics, git failures) are
/// appended there, so a GUI launched from a desktop launcher still leaves a
/// log on disk (stderr alone is invisible for a launcher-started app).
static LOG_FILE: std::sync::OnceLock<Option<Mutex<std::fs::File>>> = std::sync::OnceLock::new();

/// Append one line to the trace log file, if configured.
fn log_file_append(line: &str) {
    if let Some(Some(file)) = LOG_FILE.get() {
        if let Ok(mut file) = file.lock() {
            use std::io::Write;
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

/// Open the `KM_LOG_FILE` (once); the parent directory is created so a
/// fresh path like `~/km/km.log` works on first run.
fn log_file_init() {
    LOG_FILE.get_or_init(|| {
        std::env::var("KM_LOG_FILE").ok().and_then(|path| {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map(Mutex::new)
                .map_err(|e| {
                    eprintln!("cachyos-kernel-manager: cannot open KM_LOG_FILE {path:?}: {e}")
                })
                .ok()
        })
    });
}

/// A trace line (stderr + the log file): what the app is DOING — every
/// semantic message, state transition, effect, background-task lifecycle,
/// probe command + result, and pipeline step. This is how we find out what
/// the app is doing in the VM instead of guessing (the user's requirement).
macro_rules! vlog {
    ($($arg:tt)*) => {
        if crate::app::VERBOSE.load(std::sync::atomic::Ordering::Relaxed) {
            let line = format!(
                "[km] {:?}: {}\n",
                crate::app::START.get_or_init(std::time::Instant::now).elapsed(),
                format!($($arg)*)
            );
            eprint!("{line}");
            crate::app::log_file_append(&line);
        }
    };
}

/// An important diagnostic (stderr + the log file) that is emitted even
/// WITHOUT verbose mode — probe timeouts, background-task panics, git
/// failures, artifact-install skips. `KM_LOG_FILE` captures them on disk.
macro_rules! km_eprintln {
    ($($arg:tt)*) => {
        let line = format!($($arg)*);
        eprintln!("{line}");
        let line = format!("{line}\n");
        crate::app::log_file_append(&line);
    };
}

/// The CachyOS green accent (#00a88f) for EVERY fluent widget (combo
/// selections, focus lines, checkboxes): the fluent style derives its accent
/// from `SlintContext.accent_color` via `accentify()`. Without this, the
/// accent is either the BLUE fallback (no XDG portal) or the VM's KDE accent
/// (the portal's settings watcher overwrites the startup value), both of
/// which render the widgets light blue.
///
/// Called at startup AND on every UI sync (the XDG settings watcher can set
/// the accent asynchronously after startup; re-applying on sync keeps the
/// CachyOS green in control).
fn set_cachyos_accent() {
    let result = i_slint_core::context::with_global_context(
        || Err(i_slint_core::platform::PlatformError::NoPlatform),
        |ctx| {
            ctx.set_accent_color(i_slint_core::graphics::Color::from_rgb_u8(0x00, 0xa8, 0x8f));
            ctx.accent_color()
        },
    );
    if let Ok(color) = result {
        vlog!("accent color = {color:?}");
    }
}

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
    /// Verbose tracing on stderr (`KM_VERBOSE=1` env or `--verbose` arg).
    /// `-v` is NOT used: the shipped binary already treats `-v` as
    /// `--version` (`src/main.rs`).
    pub verbose: bool,
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
        let env_verbose = std::env::var("KM_VERBOSE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let arg_verbose = std::env::args().any(|a| a == "--verbose");
        Flags {
            home,
            system_locale,
            config_path: "/etc/scx_loader.toml".to_string(),
            aur_enabled: false,
            verbose: env_verbose || arg_verbose,
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

impl Default for CatalogPayload {
    /// The empty catalog (the oracle's "No kernels found" state): what the
    /// NullAlpm dev build produces and what a failed discovery task falls
    /// back to instead of hanging the app.
    fn default() -> Self {
        CatalogPayload {
            rows: Vec::new(),
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        }
    }
}

/// Run discovery: the real libalpm backend with the `alpm` feature, an
/// EMPTY catalog otherwise (CI/dev — the oracle's "No kernels found" path).
/// Returns `Err` on ALPM init failure — audit P1: the old `.expect()`
/// panicked and the fail-open `blocking` fallback turned the panic into a
/// valid-looking EMPTY catalog ("discovery succeeded, zero kernels"). An
/// init failure is a TASK FAILURE, surfaced as the oracle's
/// "Failed to initialize alpm handle" dialog, never a successful empty
/// probe.
#[cfg(feature = "alpm")]
pub fn run_discovery(flags: &Flags) -> Result<CatalogPayload, String> {
    use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
    use cachyos_kernel_manager_alpm::pacman_conf::MiniIni;
    use cachyos_kernel_manager_alpm::{register_sections, Alpm};
    use cachyos_kernel_manager_core::discovery::SyncDb;
    use cachyos_kernel_manager_core::DbPackage;

    struct RealAlpm<'a>(&'a AlpmHandle);
    impl Alpm for RealAlpm<'_> {
        fn sync_dbs(&self) -> Vec<SyncDb> {
            // The oracle's discovery order (`Kernel::get_kernels`,
            // kernel.cpp:184-198) iterates the needles SEARCH RESULTS in
            // `alpm_db_search` order per db — NOT `alpm_db_get_pkgcache`
            // order (the two differ; the CachyOS libalpm's search list is
            // the reverse pkgcache order — verified 2026-08-23 by the
            // ui/gui-drive court: the frozen Qt tree's row order is the
            // exact reverse of the pkgcache iteration). The discovery model
            // still needs the FULL package list for kernel/companion name
            // lookups, so the db packages are assembled as the needle
            // matches in search order first, then the remaining packages in
            // pkgcache order (lookups are order-independent).
            const HEADERS_NEEDLE: &str = "linux[^ ]*-headers";
            self.0
                .syncdb_names()
                .into_iter()
                .map(|name| {
                    let to_db = |p: cachyos_kernel_manager_alpm::ffi::DbPkg| DbPackage {
                        name: p.name,
                        version: p.version,
                    };
                    let mut packages: Vec<DbPackage> = self
                        .0
                        .db_search(&name, HEADERS_NEEDLE)
                        .into_iter()
                        .map(to_db)
                        .collect();
                    let matched: std::collections::BTreeSet<String> =
                        packages.iter().map(|p| p.name.clone()).collect();
                    for p in self.0.db_packages(&name) {
                        if !matched.contains(&p.name) {
                            packages.push(to_db(p));
                        }
                    }
                    SyncDb { name, packages }
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
    let handle = match AlpmHandle::init("/", "/var/lib/pacman/") {
        Ok(h) => h,
        Err(e) => return Err(e.to_string()),
    };
    for name in &sections {
        handle.register_syncdb(name);
    }
    let mut payload = discover_from(&RealAlpm(&handle), flags);
    // The REAL hardware profile: the production GUI must feed the plan the
    // same facts the oracle's static init collects (`kernel.cpp:41-52` + the
    // install-time module probes `kernel.cpp:114-115`) — findmnt, chwd, the
    // module-family package queries, and the LOCAL database for the
    // installed set. The old code filled `installed` from the SYNC repos
    // (every package that EXISTS upstream, not what is installed) and left
    // every probe at its default (review seam #1).
    payload.hardware = hardware_profile(&handle);
    Ok(payload)
}

/// The oracle's static-init hardware facts, probed with the exact courted
/// commands (the `findmnt`/`chwd`/`pacman -Qqs` pipelines of `kernel.cpp`
/// and the plan tool's constants) + the local database's installed set.
#[cfg(feature = "alpm")]
fn hardware_profile(handle: &cachyos_kernel_manager_alpm::ffi::AlpmHandle) -> HardwareProfile {
    let findmnt = exec_probe("findmnt -ln -o FSTYPE /");
    let root_on_zfs = findmnt.trim() == "zfs";
    // the oracle evaluates TWO separate chwd lambdas; one deterministic run
    // with the same derivation is semantically equivalent
    let chwd = exec_probe("chwd --list-installed -d 2>/dev/null | grep Name | awk '{print $4}'");
    let nvidia = exec_probe("pacman -Qqs '^linux-cachyos.*-nvidia$' 2>/dev/null");
    let nvidia_open = exec_probe("pacman -Qqs '^linux-cachyos.*-nvidia-open$' 2>/dev/null");
    // the LOCAL database is the authoritative installed set (the plan's
    // `installed` membership is about the machine, not the repos)
    let installed: std::collections::BTreeSet<String> = handle
        .local_packages()
        .into_iter()
        .map(|p| p.name)
        .collect();
    HardwareProfile {
        root_on_zfs,
        chwd_nvidia: chwd.lines().any(|l| l.starts_with("nvidia-dkms")),
        chwd_nvidia_open: chwd.lines().any(|l| l.starts_with("nvidia-open-dkms")),
        installed,
        nvidia_modules_installed: !nvidia.trim().is_empty(),
        nvidia_open_modules_installed: !nvidia_open.trim().is_empty(),
    }
}

/// Refresh ONLY the MUTABLE machine facts at the transaction-planning
/// boundary (audit P1 TOCTOU): the oracle keeps root-on-ZFS + the chwd
/// profiles as process-lifetime `static const` facts (kernel.cpp:43-52) but
/// re-queries the LIVE local database (nvidia-dkms / nvidia-open-dkms
/// presence, companion removal) and the `pacman -Qqs` module-family probes
/// at `Kernel::install()`/`remove()` time (kernel.cpp:105-130,143-161). The
/// old code fed the plan the CACHED discovery snapshot, so a package
/// operation between catalog display and Execute could choose stale
/// NVIDIA/header/companion actions. The startup-static facts pass through
/// unchanged.
#[cfg(feature = "alpm")]
fn refresh_mutable_hardware(cached: &HardwareProfile) -> HardwareProfile {
    use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
    let nvidia = exec_probe("pacman -Qqs '^linux-cachyos.*-nvidia$' 2>/dev/null");
    let nvidia_open = exec_probe("pacman -Qqs '^linux-cachyos.*-nvidia-open$' 2>/dev/null");
    // the live local-db installed set; an init failure keeps the cached set
    // (the alpm init is re-attempted for the change-check right after — the
    // oracle's parse_alpm there too, km-window.cpp:142)
    let installed = match AlpmHandle::init("/", "/var/lib/pacman/") {
        Ok(handle) => handle
            .local_packages()
            .into_iter()
            .map(|p| p.name)
            .collect::<std::collections::BTreeSet<String>>(),
        Err(_) => cached.installed.clone(),
    };
    HardwareProfile {
        installed,
        nvidia_modules_installed: !nvidia.trim().is_empty(),
        nvidia_open_modules_installed: !nvidia_open.trim().is_empty(),
        ..cached.clone()
    }
}

/// Non-libalpm build: no live refresh possible — the cached facts are the
/// only facts (CI/dev, NullAlpm).
#[cfg(not(feature = "alpm"))]
fn refresh_mutable_hardware(cached: &HardwareProfile) -> HardwareProfile {
    cached.clone()
}

/// Run one of the oracle's probe pipelines (`sh -c`, stdout captured via a
/// temp file; a failed probe yields an empty string, like the oracle's
/// error path). BOUNDED: a probe must never hang the discovery — the VM
/// experience showed `chwd`-style probes blocking forever, leaving the
/// progress dialog up with no kernels (a 15s cap + kill).
///
/// The temp capture file name is unique per call (see [`run_probe`]) so a
/// probe can never clobber another probe's capture file.
#[cfg(feature = "alpm")]
fn exec_probe(cmd: &str) -> String {
    static PROBE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let out = std::env::temp_dir().join(format!(
        "km-probe-{}-{}-{}.out",
        std::process::id(),
        PROBE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        cmd.len()
    ));
    // O_EXCL (create_new) + 0600, like [`run_probe`] (audit P1/security: the
    // old `File::create` followed a pre-created symlink and truncated an
    // arbitrary victim-writable file; the unique name + exclusive creation
    // also stops concurrent probes clobbering each other).
    use std::os::unix::fs::OpenOptionsExt;
    let stdout = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&out)
    {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut child = match std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(stdout)
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            let _ = std::fs::remove_file(&out);
            return String::new();
        }
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => break,
        }
        if std::time::Instant::now() > deadline {
            km_eprintln!("cachyos-kernel-manager: probe timed out (15s): {cmd}");
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    // pop ONE trailing newline exactly like the oracle's `utils::exec`
    let text = match text.strip_suffix('\n') {
        Some(t) => t.to_string(),
        None => text,
    };
    vlog!(
        "probe: {cmd:?} -> {}",
        text.chars().take(200).collect::<String>()
    );
    text
}

/// Non-libalpm build: an empty catalog (CI/dev without system libalpm).
/// This is a GENUINE empty result (the oracle would show the "No kernels
/// found!" dialog), not a failure.
#[cfg(not(feature = "alpm"))]
pub fn run_discovery(flags: &Flags) -> Result<CatalogPayload, String> {
    use cachyos_kernel_manager_alpm::NullAlpm;
    Ok(discover_from(&NullAlpm::default(), flags))
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

/// A file-path entry dialog (Slint has no native picker; the oracle's
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

/// The UI-level message: semantic UI messages (the courted vocabulary)
/// plus rendering-only messages (task results, dialogs, widgets).
#[derive(Debug, Clone)]
pub enum UiMessage {
    /// The semantic message vocabulary (crates/.../lib.rs).
    Semantic(Message),
    /// A discovery pass finished (initial load or post-transaction refresh).
    CatalogLoaded(Box<CatalogPayload>),
    /// Discovery FAILED (an ALPM init error or a worker panic) — audit P1:
    /// the old `blocking` degraded a panic to `CatalogPayload::default()`, a
    /// valid-looking EMPTY catalog that silently turned an ALPM failure into
    /// "discovery succeeded, zero kernels". The handler shows the oracle's
    /// init-failure dialog and readies the app with an empty catalog (the
    /// OK stays disabled), never pretending the probe succeeded.
    DiscoveryFailed(String),
    /// The Configure-window prepare FAILED (a worker panic) — audit P1: a
    /// panic must not masquerade as an empty patch list. The handler shows
    /// the error dialog and closes the Configure flow (state -> Closed), the
    /// oracle's clone-failure behavior without the success illusion.
    ConfigureFailed(String),
    /// A generic background-task failure (a panic) that has no dedicated
    /// message: a plain error dialog; the task's own state path (if any) is
    /// untouched. The panic text is also in the km log.
    TaskFailed(String),
    /// The transaction worker started the commit run (the phase projection
    /// enters `TransactionRunning` for the real work).
    TransactionStarted,
    /// The transaction commit finished; `changed` = the kernels-change flag.
    TransactionFinished {
        changed: bool,
    },
    /// The transaction failed (the alpm change-check).
    TransactionFailed {
        message: String,
    },
    /// A config load/save I/O error — a PLAIN error dialog, never a
    /// transaction-state transition (config I/O is not a pacman
    /// transaction; routing it through TransactionFailed used to soft-lock
    /// the OK button).
    ConfigError(String),
    /// The Configure-window prepare flow (git refresh + the patches-tab
    /// source-array probe) finished; the payload is the `.patch`-filtered
    /// source array (the oracle's `reset_patches_data_tab` result).
    /// `generation` is the operation epoch captured at dispatch: a rapid
    /// variant/mutation that bumps the epoch BEFORE this lands makes the
    /// result stale (a newer probe is already in flight), so the update
    /// handler discards it (audit P1 — the old result carried no
    /// fingerprint and clobbered newer patches).
    ConfigurePrepared {
        generation: u64,
        patches: Vec<String>,
    },
    /// A patches-tab refresh probe finished (variant/lto/nvidia-open
    /// changes re-run `reset_patches_data_tab`). `generation` is the
    /// operation epoch: a stale completion (a rapid A→B change whose
    /// A-probe finishes last) is discarded by the update handler (audit P1:
    /// the old result carried no fingerprint and blindly replaced the list).
    PatchesRefreshed {
        generation: u64,
        patches: Vec<String>,
    },
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
    /// The remaining option combos (review seam #9: these were displayed but
    /// wired to `DialogDismissed` no-ops — now they feed the model).
    TicklessPicked(cachyos_kernel_manager_core::options::TicklessMode),
    HugepagePicked(cachyos_kernel_manager_core::options::HugepageMode),
    CpuOptPicked(cachyos_kernel_manager_core::options::CpuOptMode),
    /// The sched-ext window was closed (hides only that window).
    ScxWindowClosed,
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

// The sched-ext minimum-size clamp timer. The declared 480x320 minimum lives
// on the CONTENT root of scx_window.slint (the content-layout path is the one
// proven to reach the WM; the window-level constraint mangled on some WMs).
// As a belt-and-suspenders fallback, while the sched-ext window is visible a
// repeating timer re-clamps it to 480x320 logical if it's ever dragged below
// (the 30ms cadence makes the correction imperceptible during a drag).
// `None` while hidden.
//
// This lives in a thread_local, NOT in `App`: `slint::Timer` is `!Send` and
// `App` is shared behind `Arc<Mutex<App>>` with the probe worker threads. The
// Slint event loop runs on ONE thread, so the thread_local is safe, and the
// timer does NOT keep the event loop alive — closing the last window calls
// `quit_event_loop`, which hard-exits regardless of pending timers
// (i-slint-backend-winit event_loop.rs `CustomEvent::Exit`).
thread_local! {
    static SCX_CLAMP: RefCell<Option<slint::Timer>> = const { RefCell::new(None) };
}

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
    /// The Slint windows (the presentation; default weaks in the tests).
    /// The dialogs (progress/error/confirm/file-path) are OVERLAYS inside
    /// these windows, never separate OS windows (they would show up as
    /// taskbar entries — review "four windows" issue).
    ui: slint::Weak<MainWindow>,
    /// The Configure window (shown while `configuration != Closed`).
    configure_window: slint::Weak<ConfigureWindow>,
    /// The sched-ext window (shown while `scx == Visible`).
    scx_window: slint::Weak<SchedExtWindow>,
    /// The patches-tab refresh operation generation (audit P1): incremented
    /// SYNCHRONOUSLY at each refresh dispatch; a completion whose generation
    /// is stale is discarded (rapid variant changes must not let an older
    /// probe's result clobber the newer selection's patches).
    patch_epoch: u64,
    /// The Configure-window build's cancellation contract (audit P0 — the
    /// oracle does NOT destroy a QProcess on close, so terminating the
    /// in-flight build is an explicit INTENTIONAL_CORRECTION, see
    /// KNOWN_DIVERGENCES D-008; the VM oracle court proves the difference).
    ///
    /// This is an OPERATION-GENERATION token, not a reusable boolean: each
    /// BuildRequested increments it SYNCHRONOUSLY at dispatch (run_effect),
    /// and the worker captures the generation it was born with. A
    /// Configure cancel/close bumps it again (invalidating the in-flight
    /// worker) and kills the owned terminal-helper child. The worker checks
    /// its generation before spawning AND after the child is stored, so a
    /// cancel that lands in either window aborts the build — no race where
    /// the worker overwrites the flag or spawns after the cancel saw no
    /// child (audit P0: the old reusable boolean had both races).
    build_epoch: Arc<AtomicU64>,
    /// The in-flight terminal-helper child (the build's owned process
    /// handle; `None` while no build/install runs). The cancel path takes +
    /// kills it so the worker's wait returns and reports the failure branch.
    build_proc: Arc<Mutex<Option<std::process::Child>>>,
}

impl App {
    /// A new app with no live windows (the tests use this). Runs the
    /// startup event (the discovery-progress state) and returns its task.
    pub fn new(flags: Flags) -> (App, Task) {
        VERBOSE.store(flags.verbose, std::sync::atomic::Ordering::Relaxed);
        log_file_init();
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
            ui: slint::Weak::default(),
            configure_window: slint::Weak::default(),
            scx_window: slint::Weak::default(),
            patch_epoch: 0,
            build_epoch: Arc::new(AtomicU64::new(0)),
            build_proc: Arc::new(Mutex::new(None)),
        };
        let task = app.on_event(AppEvent::Started);
        (app, task)
    }

    /// A new app bound to the live Slint windows (`run` uses this).
    #[allow(clippy::too_many_arguments)] // the three window weaks mirror the oracle's windows
    pub fn with_windows(
        flags: Flags,
        ui: slint::Weak<MainWindow>,
        configure: slint::Weak<ConfigureWindow>,
        scx: slint::Weak<SchedExtWindow>,
    ) -> (App, Task) {
        let (mut app, task) = App::new(flags);
        app.ui = ui;
        app.configure_window = configure;
        app.scx_window = scx;
        (app, task)
    }

    /// The semantic message → core event mapping + the UI-side model updates
    /// that precede it (patch ops, variant switch, config load).
    fn on_semantic(&mut self, message: Message) -> Task {
        match message {
            Message::VariantChanged { variant } => {
                // `main_combo_box` change handler + reset_patches_data_tab
                // (conf-window.cpp:553-602); the source-array probe is a
                // UI-side action (the app's git cache) — empty here. The
                // core build_options must follow (the RunBuild effect's
                // variant_dir comes from it).
                self.configure.on_variant_changed(variant, &[]);
                self.state.build_options.variant = variant;
                Task::None
            }
            Message::PatchAdded { entry } => {
                self.configure.add_remote_patch(entry);
                Task::None
            }
            Message::PatchRemoved { index } => {
                self.configure.remove_patch(index);
                Task::None
            }
            Message::PatchMoved { from, to } => {
                // the list widget's move ops are up/down only (courted); the
                // semantic message carries the final index
                let _ = (from, to);
                Task::None
            }
            Message::ConfigLoaded { config } => {
                let outdated = self.configure.load_config(&config);
                if outdated {
                    self.state.dialogs = DialogsState::Error {
                        message: self.tr(tr_ctx::CONF, "Config file(%1) is outdated"),
                    };
                }
                Task::None
            }
            Message::SchedulerChanged { scheduler, mode } => {
                if let Some(scx) = &mut self.scx {
                    let _ = mode;
                    let config = cachyos_kernel_manager_scx::config::default_config();
                    scx.on_sched_changed(&scheduler, &config);
                }
                Task::None
            }
            Message::KernelToggled { raw } => {
                // resolve the STABLE identity to the discovery-order row
                // (the core selection is index-based; the UI never sends
                // presentation indices across this boundary)
                let Some(row) = self.rows.iter().position(|r| r.raw == raw) else {
                    return Task::None;
                };
                let task = self.on_event(AppEvent::KernelToggled { row });
                // the checkbox projection is DERIVED from the authoritative
                // core selection, never an independently mutable copy
                if let (Some(view), Some(core)) =
                    (self.rows.get_mut(row), self.state.selection.rows.get(row))
                {
                    view.checked = core.checked;
                }
                // the DISPLAYED rows carry the same checked state (they are
                // the recompute_sort base — the oracle mutates the tree item
                // in place, km-window.cpp:285-293)
                if let Some(sorted) = self.sorted_rows.iter_mut().find(|r| r.raw == raw) {
                    sorted.checked = self.rows[row].checked;
                }
                self.recompute_sort();
                task
            }
            Message::ExecuteRequested => self.on_event(AppEvent::ExecuteRequested),
            Message::ConfigureRequested => self.on_event(AppEvent::ConfigureRequested),
            Message::BuildRequested => self.on_event(AppEvent::BuildRequested),
            Message::InstallArtifactsRequested => {
                self.on_event(AppEvent::InstallArtifactsRequested)
            }
            Message::CloseRequested => self.on_event(AppEvent::CloseRequested),
            // the MAIN window's Cancel closes the app (the oracle's
            // km-window Cancel == close); the Configure window has its own
            // distinct event (ConfigurationCancelRequested)
            Message::CancelRequested => self.on_event(AppEvent::CloseRequested),
            Message::ConfigurationCancelRequested => {
                self.on_event(AppEvent::ConfigurationCancelRequested)
            }
            Message::ConfigurationCloseRequested => {
                self.on_event(AppEvent::ConfigurationCloseRequested)
            }
            Message::SchedextRequested => self.on_event(AppEvent::ScxShowRequested),
            Message::ScxCloseRequested => self.on_event(AppEvent::ScxWindowClosed),
        }
    }

    /// Run one core event through the courted transition; execute the
    /// resulting effects.
    fn on_event(&mut self, event: AppEvent) -> Task {
        vlog!("event: {event:?}");
        let (next, effects) = transition(&self.state, event);
        self.state = next;
        vlog!(
            "state -> phase {:?} ({} effects)",
            self.state.phase(),
            effects.len()
        );
        let mut tasks = Vec::new();
        for effect in effects {
            if let Some(task) = self.run_effect(effect) {
                tasks.push(task);
            }
        }
        Task::Batch(tasks)
    }

    /// Interpret one courted effect as a runtime action.
    fn run_effect(&mut self, effect: Effect) -> Option<Task> {
        vlog!("effect: {effect:?}");
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
            Effect::RunBuild { .. } => {
                // establish the operation generation SYNCHRONOUSLY at
                // dispatch (before any worker runs): the worker born with
                // this value aborts if a cancel bumps the epoch (audit P0)
                let epoch = self
                    .build_epoch
                    .fetch_add(1, Ordering::Relaxed)
                    .wrapping_add(1);
                Some(self.build_task(epoch))
            }
            Effect::InstallArtifacts => {
                // the install is owned by the SAME operation: it carries the
                // current generation (a cancel before it spawns aborts it)
                let epoch = self.build_epoch.load(Ordering::Relaxed);
                Some(self.artifacts_task(epoch))
            }
            Effect::ShowScxWindow => Some(self.scx_init_task()),
            Effect::Close => Some(Task::Exit), // the oracle's closeEvent exits the app
        }
    }

    /// Push the current app state into the Slint windows (the presentation
    /// sync; a no-op with the default weaks — the tests).
    fn sync_ui(&self) {
        // keep the CachyOS green accent in control (the XDG settings watcher
        // may have overridden it with the VM's KDE accent)
        set_cachyos_accent();
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let rows: Vec<TreeRow> = self
            .sorted_rows
            .iter()
            .map(|r| TreeRow {
                raw: r.raw.clone().into(),
                version: r.version_text.clone().into(),
                category: r.category.clone().into(),
                checked: r.checked,
                immutable: r.immutable,
            })
            .collect();
        ui.set_rows(ModelRc::new(slint::VecModel::from(rows)));
        // the description + tree headers resolve through tr() like the
        // oracle's (the MainWindow context catalogs translate the whole
        // HTML block and the four column headers — audit P2: the old sync
        // fed the English constants straight through, so a German session
        // showed English headers); the catalog key for the description is
        // the RAW HTML literal (km-window.ui:27), the view strips it.
        ui.set_description(
            strings::main_description_plain(&self.tr(tr_ctx::MAIN, strings::MAIN_DESCRIPTION_HTML))
                .into(),
        );
        ui.set_execute_enabled(self.state.execute_enabled());
        ui.set_schedext_visible(self.schedext_button_visible());
        ui.set_label_choose(self.tr(tr_ctx::MAIN, strings::tree_columns::CHOOSE).into());
        ui.set_label_pkgname(
            self.tr(tr_ctx::MAIN, strings::tree_columns::PKG_NAME)
                .into(),
        );
        ui.set_label_version(self.tr(tr_ctx::MAIN, strings::tree_columns::VERSION).into());
        ui.set_label_category(
            self.tr(tr_ctx::MAIN, strings::tree_columns::CATEGORY)
                .into(),
        );
        ui.set_label_execute(self.tr(tr_ctx::MAIN, strings::main_buttons::EXECUTE).into());
        ui.set_label_configure(
            self.tr(tr_ctx::MAIN, strings::main_buttons::CONFIGURE)
                .into(),
        );
        ui.set_label_cancel(self.tr(tr_ctx::MAIN, strings::main_buttons::CANCEL).into());
        ui.set_label_schedext(
            self.tr(tr_ctx::MAIN, strings::main_buttons::SCHED_EXT)
                .into(),
        );
        // the shared dialog overlay (progress/error/confirm/file-path are
        // IN-window overlays — no separate OS windows, no taskbar entries)
        let (pv, pm, ev, em, cv, cm, pav, pat, pavv) = self.dialog_overlay_values();
        ui.set_dialog_progress_visible(pv);
        ui.set_dialog_progress_message(pm.into());
        ui.set_dialog_error_visible(ev);
        ui.set_dialog_error_message(em.into());
        ui.set_dialog_confirm_visible(cv);
        ui.set_dialog_confirm_message(cm.into());
        ui.set_dialog_path_visible(pav);
        ui.set_dialog_path_title(pat.into());
        ui.set_dialog_path_value(pavv.into());
        // the standard dialog buttons (Qt's own translations)
        ui.set_dialog_button_ok(self.standard_button(StandardButton::Ok).into());
        ui.set_dialog_button_yes(self.standard_button(StandardButton::Yes).into());
        ui.set_dialog_button_no(self.standard_button(StandardButton::No).into());
        ui.set_dialog_button_cancel(self.standard_button(StandardButton::Cancel).into());
        // the Configure/SchedExt windows follow their states
        self.sync_configure_window();
        self.sync_scx_window();
    }

    /// The active dialog state as overlay values `(progress_visible,
    /// progress_message, error_visible, error_message, confirm_visible,
    /// confirm_message, path_visible, path_title, path_value)` — pushed into
    /// EVERY window's overlay so the dialog appears on whichever window is on
    /// top (e.g. the install question over the Configure window).
    fn dialog_overlay_values(
        &self,
    ) -> (
        bool,
        String,
        bool,
        String,
        bool,
        String,
        bool,
        String,
        String,
    ) {
        let (progress, error, confirm) = match &self.state.dialogs {
            DialogsState::Progress { message } => (Some(message.clone()), None, None),
            DialogsState::Error { message } => (None, Some(message.clone()), None),
            DialogsState::Confirm { message } => (None, None, Some(message.clone())),
            DialogsState::None => (None, None, None),
        };
        let (path_visible, path_title, path_value) = match &self.path_dialog {
            Some(d) => (true, d.title.clone(), d.value.clone()),
            None => (false, String::new(), String::new()),
        };
        (
            progress.is_some(),
            progress.unwrap_or_default(),
            error.is_some(),
            error.unwrap_or_default(),
            confirm.is_some(),
            confirm.unwrap_or_default(),
            path_visible,
            path_title,
            path_value,
        )
    }

    /// The user's translated string (Qt `tr()` on the current locale).
    pub fn tr(&self, context: &str, source: &str) -> String {
        self.locale.tr(context, source).to_string()
    }

    /// Qt's OWN standard-button text for the resolved catalog (the qtbase
    /// QMessageBox buttons; English fallback for an unresolvable locale).
    pub fn standard_button(&self, button: StandardButton) -> &'static str {
        strings::standard_buttons::text(self.locale.catalog, button)
    }

    /// Push the Configure window: visibility (shown while Preparing/Editing),
    /// the variant combo, the 11 option checkboxes, the six option combos,
    /// the custom name, and the patches tab.
    fn sync_configure_window(&self) {
        let Some(w) = self.configure_window.upgrade() else {
            return;
        };
        let open = matches!(
            self.state.configuration,
            ConfigurationState::Preparing | ConfigurationState::Editing
        );
        if open {
            let c = &self.configure;
            // the variant combo (the 10 courted labels). The oracle builds
            // them with `ConfWindow::tr()` (conf-window.cpp:487-488) — the
            // ConfWindow catalog context, NOT ConfOptionsPage (audit P2: the
            // old sync resolved them in the options-page context; all
            // variant translations are empty/unfinished in the frozen .ts,
            // so the resolved text is the same today, but the context must
            // match the oracle's lookup exactly).
            let variants: Vec<slint::SharedString> =
                cachyos_kernel_manager_core::options::KernelVariant::ALL
                    .iter()
                    .map(|v| {
                        self.tr(tr_ctx::CONF, crate::configure_window::variant_label(*v))
                            .into()
                    })
                    .collect();
            let variant_index = cachyos_kernel_manager_core::options::KernelVariant::ALL
                .iter()
                .position(|v| *v == c.variant)
                .unwrap_or(0) as i32;
            w.set_variant_labels(ModelRc::new(slint::VecModel::from(variants)));
            w.set_variant_index(variant_index);
            // the option checkboxes, split into the two presentation groups
            // (top 8 above the combos, bottom 3 below — the ORACLE's order,
            // see conf_check_at). Two models, NOT one with a `visible:`
            // filter: Slint 1.17 lowers a runtime `visible` binding to a
            // Clip wrapper that still occupies layout space, leaving gaps.
            let checks: Vec<ConfCheckRow> = {
                let zfs_enabled = c.switch.zfs_enabled;
                let mk = |label: &str, checked: bool, enabled: bool| ConfCheckRow {
                    label: self.tr(tr_ctx::CONF_OPTIONS, label).into(),
                    checked,
                    enabled,
                };
                vec![
                    mk("Enable CachyOS config", c.switch.cachy_config_checked, true),
                    mk(
                        "Tweak kernel options prior to a build via nconfig",
                        c.nconfig_checked,
                        true,
                    ),
                    mk(
                        "Tweak kernel options prior to a build via xconfig",
                        c.xconfig_checked,
                        true,
                    ),
                    mk("Use Modprobed-db", c.localmodcfg_checked, true),
                    mk(
                        "Use the current kernel's config",
                        c.use_current_checked,
                        true,
                    ),
                    mk("Enable KBUILD_CFLAGS -O3", c.hardly_checked, true),
                    mk(
                        "Set performance governor as default",
                        c.per_gov_checked,
                        true,
                    ),
                    mk("Enable TCP_CONG_BBR3", c.tcp_bbr3_checked, true),
                    mk("Build the ZFS module", c.switch.zfs_checked, zfs_enabled),
                    mk(
                        "Build the open NVIDIA module",
                        c.builtin_nvidia_open_checked,
                        true,
                    ),
                    mk(
                        "Include vmlinux with debug informations/symbols",
                        c.build_debug_checked,
                        true,
                    ),
                ]
            };
            let (checks_top, checks_bottom): (Vec<ConfCheckRow>, Vec<ConfCheckRow>) = {
                let (top, bottom) = checks.split_at(8);
                (top.to_vec(), bottom.to_vec())
            };
            w.set_checks_top(ModelRc::new(slint::VecModel::from(checks_top)));
            w.set_checks_bottom(ModelRc::new(slint::VecModel::from(checks_bottom)));
            // the option combos (items + current index)
            let lto_items: Vec<slint::SharedString> = c
                .switch
                .lto_items
                .iter()
                .map(|m| lto_label(*m).into())
                .collect();
            let lto_index = c
                .switch
                .lto_items
                .iter()
                .position(|m| *m == c.switch.lto_selected)
                .unwrap_or(0) as i32;
            w.set_lto_items(ModelRc::new(slint::VecModel::from(lto_items)));
            w.set_lto_index(lto_index);
            let preempt_items: Vec<slint::SharedString> = c
                .switch
                .preempt_items
                .iter()
                .map(|m| preempt_label(*m).into())
                .collect();
            let preempt_index = c
                .switch
                .preempt_items
                .iter()
                .position(|m| *m == c.switch.preempt_selected)
                .unwrap_or(0) as i32;
            w.set_preempt_items(ModelRc::new(slint::VecModel::from(preempt_items)));
            w.set_preempt_index(preempt_index);
            let hz_items: Vec<slint::SharedString> = strings::combo_options::HZ_TICKS
                .iter()
                .map(|s| (*s).into())
                .collect();
            let hz_index = hz_index_of(c.switch.hz_selected) as i32;
            w.set_hz_items(ModelRc::new(slint::VecModel::from(hz_items)));
            w.set_hz_index(hz_index);
            let tickless_items: Vec<slint::SharedString> = strings::combo_options::TICKLESS
                .iter()
                .map(|s| (*s).into())
                .collect();
            let tickless_index = tickless_index_of(c.tickless) as i32;
            w.set_tickless_items(ModelRc::new(slint::VecModel::from(tickless_items)));
            w.set_tickless_index(tickless_index);
            let hugepage_items: Vec<slint::SharedString> = strings::combo_options::HUGE_PAGE
                .iter()
                .map(|s| (*s).into())
                .collect();
            let hugepage_index = hugepage_index_of(c.hugepage) as i32;
            w.set_hugepage_items(ModelRc::new(slint::VecModel::from(hugepage_items)));
            w.set_hugepage_index(hugepage_index);
            let cpuopt_items: Vec<slint::SharedString> = strings::combo_options::CPU_OPT
                .iter()
                .map(|s| (*s).into())
                .collect();
            let cpuopt_index = cpuopt_index_of(c.cpu_opt) as i32;
            w.set_cpuopt_items(ModelRc::new(slint::VecModel::from(cpuopt_items)));
            w.set_cpuopt_index(cpuopt_index);
            w.set_custom_name(c.custom_name.clone().into());
            let patches: Vec<slint::SharedString> =
                c.patches.iter().map(|p| p.clone().into()).collect();
            w.set_patches(ModelRc::new(slint::VecModel::from(patches)));
            let selected = w
                .get_selected_patch()
                .min(c.patches.len().saturating_sub(1) as i32)
                .max(0);
            w.set_selected_patch(selected);
            w.set_build_running(self.state.build.in_flight());
            // the translated labels
            w.set_label_variant(self.tr(tr_ctx::CONF_OPTIONS, "Select kernel").into());
            w.set_label_custom_name(self.tr(tr_ctx::CONF_OPTIONS, "Custom package name").into());
            w.set_label_hz(self.tr(tr_ctx::CONF_OPTIONS, "Running tick rate").into());
            w.set_label_tickless(self.tr(tr_ctx::CONF_OPTIONS, "Select tickless").into());
            w.set_label_preempt(self.tr(tr_ctx::CONF_OPTIONS, "Select preempt").into());
            w.set_label_hugepage(
                self.tr(tr_ctx::CONF_OPTIONS, "Transparent Hugepages")
                    .into(),
            );
            w.set_label_cpuopt(
                self.tr(tr_ctx::CONF_OPTIONS, "CPU compiler optimizations")
                    .into(),
            );
            w.set_label_lto(self.tr(tr_ctx::CONF_OPTIONS, "Enable LTO").into());
            w.set_label_tab_options(self.tr(tr_ctx::CONF, "Options").into());
            w.set_label_tab_patches(self.tr(tr_ctx::CONF, "Patches").into());
            w.set_label_add_local(self.tr(tr_ctx::CONF_PATCHES, "Add local patch").into());
            w.set_label_add_remote(self.tr(tr_ctx::CONF_PATCHES, "Add remote patch").into());
            w.set_label_remove(self.tr(tr_ctx::CONF_PATCHES, "Remove").into());
            w.set_label_up(self.tr(tr_ctx::CONF_PATCHES, "Move up").into());
            w.set_label_down(self.tr(tr_ctx::CONF_PATCHES, "Move down").into());
            w.set_label_save(self.tr(tr_ctx::CONF_OPTIONS, "Save").into());
            w.set_label_load(self.tr(tr_ctx::CONF_OPTIONS, "Load").into());
            w.set_label_cancel(self.tr(tr_ctx::CONF_OPTIONS, "Cancel").into());
            w.set_label_execute(self.tr(tr_ctx::CONF_OPTIONS, "Build kernel").into());
            // the shared dialog overlay (the install question + the file-path
            // dialog appear over the Configure window, like the oracle)
            let (pv, pm, ev, em, cv, cm, pav, pat, pavv) = self.dialog_overlay_values();
            w.set_dialog_progress_visible(pv);
            w.set_dialog_progress_message(pm.into());
            w.set_dialog_error_visible(ev);
            w.set_dialog_error_message(em.into());
            w.set_dialog_confirm_visible(cv);
            w.set_dialog_confirm_message(cm.into());
            w.set_dialog_path_visible(pav);
            w.set_dialog_path_title(pat.into());
            w.set_dialog_path_value(pavv.into());
            w.set_dialog_button_ok(self.standard_button(StandardButton::Ok).into());
            w.set_dialog_button_yes(self.standard_button(StandardButton::Yes).into());
            w.set_dialog_button_no(self.standard_button(StandardButton::No).into());
            w.set_dialog_button_cancel(self.standard_button(StandardButton::Cancel).into());
            let _ = w.show();
        } else {
            let _ = w.hide();
        }
    }

    /// Push the sched-ext window: visibility (shown while `scx == Visible`),
    /// the running label, the scheduler/profile combos, the flags, and the
    /// widget enablement.
    fn sync_scx_window(&self) {
        let Some(w) = self.scx_window.upgrade() else {
            return;
        };
        if self.state.scx == ScxState::Visible {
            if let Some(scx) = &self.scx {
                w.set_label_running(
                    self.tr(tr_ctx::SCX, strings::scx_labels::RUNNING_SCHEDULER)
                        .into(),
                );
                w.set_running(scx.running_scheduler.clone().into());
                w.set_label_scheduler(
                    self.tr(tr_ctx::SCX, strings::scx_labels::SELECT_SCHEDULER)
                        .into(),
                );
                let scheds: Vec<slint::SharedString> =
                    scx.schedulers.iter().map(|s| s.clone().into()).collect();
                w.set_schedulers(ModelRc::new(slint::VecModel::from(scheds)));
                w.set_scheduler_index(
                    scx.schedulers
                        .iter()
                        .position(|s| *s == scx.scheduler)
                        .unwrap_or(0) as i32,
                );
                w.set_label_profile(
                    self.tr(tr_ctx::SCX, strings::scx_labels::SELECT_PROFILE)
                        .into(),
                );
                let profiles: Vec<slint::SharedString> = strings::combo_options::SCX_PROFILE
                    .iter()
                    .map(|s| (*s).into())
                    .collect();
                w.set_profiles(ModelRc::new(slint::VecModel::from(profiles)));
                w.set_profile_index(
                    strings::combo_options::SCX_PROFILE
                        .iter()
                        .position(|p| *p == scx.profile)
                        .unwrap_or(0) as i32,
                );
                w.set_profile_visible(scx.profile_visible);
                w.set_label_flags(self.tr(tr_ctx::SCX, strings::scx_labels::SET_FLAGS).into());
                w.set_flags(scx.flags.clone().into());
                w.set_enabled(scx.enabled);
                w.set_label_apply(self.tr(tr_ctx::SCX, "Apply").into());
                w.set_label_disable(self.tr(tr_ctx::SCX, "Disable").into());
                w.set_label_cancel(self.tr(tr_ctx::SCX, "Cancel").into());
                // the shared dialog overlay (scx errors appear over this window)
                let (pv, pm, ev, em, cv, cm, pav, pat, pavv) = self.dialog_overlay_values();
                w.set_dialog_progress_visible(pv);
                w.set_dialog_progress_message(pm.into());
                w.set_dialog_error_visible(ev);
                w.set_dialog_error_message(em.into());
                w.set_dialog_confirm_visible(cv);
                w.set_dialog_confirm_message(cm.into());
                w.set_dialog_path_visible(pav);
                w.set_dialog_path_title(pat.into());
                w.set_dialog_path_value(pavv.into());
                w.set_dialog_button_ok(self.standard_button(StandardButton::Ok).into());
                w.set_dialog_button_yes(self.standard_button(StandardButton::Yes).into());
                w.set_dialog_button_no(self.standard_button(StandardButton::No).into());
                w.set_dialog_button_cancel(self.standard_button(StandardButton::Cancel).into());
                let _ = w.show();
                // the belt-and-suspenders min-size clamp: run while visible
                // (480x320 — must match scx_window.slint's content-root mins)
                if SCX_CLAMP.with(|c| c.borrow().is_none()) {
                    let weak = w.as_weak();
                    let timer = slint::Timer::default();
                    timer.start(
                        slint::TimerMode::Repeated,
                        std::time::Duration::from_millis(30),
                        move || {
                            if let Some(w) = weak.upgrade() {
                                let win = w.window();
                                let sf = win.scale_factor();
                                let size = win.size();
                                let (log_w, log_h) =
                                    (size.width as f32 / sf, size.height as f32 / sf);
                                if log_w < 480.0 || log_h < 320.0 {
                                    win.set_size(slint::LogicalSize::new(
                                        log_w.max(480.0),
                                        log_h.max(320.0),
                                    ));
                                }
                            }
                        },
                    );
                    SCX_CLAMP.with(|c| *c.borrow_mut() = Some(timer));
                }
            }
        } else {
            let _ = w.hide();
            // stop the clamp (dropping the timer stops it; it restarts on show)
            SCX_CLAMP.with(|c| c.borrow_mut().take());
        }
    }

    /// The sched-ext button visibility (`km-window.cpp:185-188`): the state
    /// file's existence.
    pub fn schedext_button_visible(&self) -> bool {
        std::path::Path::new("/sys/kernel/sched_ext/state").exists()
    }

    /// The current tree rows, in the current sort order.
    ///
    /// The STABLE base is the DISPLAYED order (`sorted_rows`), never the
    /// discovery catalog: Qt's `sortByColumn` re-sorts the CURRENT items, so
    /// the previous sort order is the tie-break for equal keys — the
    /// ui/gui-drive court witnesses this exactly (the frozen tree's Version
    /// sort keeps the PkgName-ascending order within equal versions, and the
    /// Category sort keeps the Version-ascending order within equal
    /// categories). A fresh catalog seeds `sorted_rows` with the discovery
    /// order (the oracle's `init_kernels`: clear + re-add with the current
    /// column's auto-sort), so the fallback is only the pre-load state.
    fn recompute_sort(&mut self) {
        let base = if self.sorted_rows.is_empty() {
            self.rows.clone()
        } else {
            self.sorted_rows.clone()
        };
        let mut rows = base;
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
        // the AUTHORITATIVE sorted order (the production-integration court's
        // identity witness: the AT-SPI tree of the accesskit 0.22.1 bridge
        // cannot survive full rebuilds in the court VMs, so the sorted row
        // sequence is witnessed from the app's own semantic trace — the same
        // courted state that drives the table)
        vlog!(
            "sort: column={} asc={} rows={:?}",
            self.sort_column,
            self.sort_ascending,
            self.sorted_rows
                .iter()
                .map(|r| r.raw.as_str())
                .collect::<Vec<_>>()
        );
    }

    /// Mirror the Configure window's model into the core `build_options`
    /// (review seam #6: the GUI must feed the plan the REAL option state —
    /// the build's `options_env_string`/`variant_dir` come from here). Called
    /// after every configure-model mutation.
    fn sync_build_options(&mut self) {
        let c = &self.configure;
        self.state.build_options = cachyos_kernel_manager_core::options::BuildOptions {
            variant: c.variant,
            hardly: c.hardly_checked,
            per_gov: c.per_gov_checked,
            tcp_bbr3: c.tcp_bbr3_checked,
            cachy_config: c.switch.cachy_config_checked,
            nconfig: c.nconfig_checked,
            xconfig: c.xconfig_checked,
            localmodcfg: c.localmodcfg_checked,
            use_current: c.use_current_checked,
            builtin_zfs: c.switch.zfs_checked,
            builtin_nvidia_open: c.builtin_nvidia_open_checked,
            build_debug: c.build_debug_checked,
            hz_ticks: c.switch.hz_selected,
            tickless: c.tickless,
            preempt: c.switch.preempt_selected,
            hugepage: c.hugepage,
            lto: c.switch.lto_selected,
            cpu_opt: c.cpu_opt,
            custom_name: c.custom_name.clone(),
        };
    }
}

/// Map a Configure-window checkbox row index to its semantic [`ConfCheck`]
/// (the `checks` model order in `configure_window.slint` — the ORACLE's
/// order from `conf-options-page.ui`: cachy_config first, hardly in the
/// middle, zfs/nvidia-open/build-debug last; the first 8 render above the
/// combos, the last 3 below them — keep in sync).
fn conf_check_at(index: usize) -> Option<ConfCheck> {
    match index {
        0 => Some(ConfCheck::CachyConfig),
        1 => Some(ConfCheck::Nconfig),
        2 => Some(ConfCheck::Xconfig),
        3 => Some(ConfCheck::Localmodcfg),
        4 => Some(ConfCheck::UseCurrent),
        5 => Some(ConfCheck::Hardly),
        6 => Some(ConfCheck::PerGov),
        7 => Some(ConfCheck::TcpBbr3),
        8 => Some(ConfCheck::Zfs),
        9 => Some(ConfCheck::NvidiaOpen),
        10 => Some(ConfCheck::BuildDebug),
        _ => None,
    }
}

/// The lto combo labels (strings::combo_options::LTO, index-aligned with
/// `LtoMode::ALL`).
fn lto_label(mode: LtoMode) -> &'static str {
    match mode {
        LtoMode::None => "No",
        LtoMode::Full => "Full",
        LtoMode::Thin => "Thin",
        LtoMode::ThinDist => "Thin-dist",
    }
}

/// The preempt combo labels (the base set + the hardened/lts extension).
fn preempt_label(mode: PreemptMode) -> &'static str {
    match mode {
        PreemptMode::Full => "Full",
        PreemptMode::Lazy => "Lazy",
        PreemptMode::Voluntary => "Voluntary",
        PreemptMode::None => "None",
    }
}

/// The combo index of a value within its `ALL` array (the label arrays are
/// index-aligned with the value arrays).
fn hz_index_of(hz: HzTick) -> usize {
    HzTick::ALL.iter().position(|h| *h == hz).unwrap_or(0)
}
fn tickless_index_of(mode: TicklessMode) -> usize {
    TicklessMode::ALL
        .iter()
        .position(|m| *m == mode)
        .unwrap_or(0)
}
fn hugepage_index_of(mode: HugepageMode) -> usize {
    HugepageMode::ALL
        .iter()
        .position(|m| *m == mode)
        .unwrap_or(0)
}
fn cpuopt_index_of(mode: CpuOptMode) -> usize {
    CpuOptMode::ALL.iter().position(|m| *m == mode).unwrap_or(0)
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

/// The courted artifact globs for a built PKGBUILD (`conf-window.cpp:218-
/// 298`): the pkgfuncs probe (`declare -F` + `pkgver:`) + the PKGEXT probe
/// (`/etc/makepkg.conf`) + the artifact-glob model. Empty when the PKGBUILD
/// is missing or the probes fail (the oracle's error path).
fn build_artifact_globs(pkgbuild: &std::path::Path) -> Vec<String> {
    use cachyos_kernel_manager_build::{
        artifact_globs, parse_pkgfuncs_probe_output, pkgext_probe_script, pkgfuncs_probe_script,
    };
    if !pkgbuild.exists() {
        return Vec::new();
    }
    let funcs = run_probe(&pkgfuncs_probe_script(), Some(pkgbuild));
    let (suffixes, version) = parse_pkgfuncs_probe_output(&funcs);
    let Some((pkgver, pkgrel)) = version else {
        return Vec::new();
    };
    if suffixes.is_empty() {
        return Vec::new();
    }
    let pkgext = run_probe(&pkgext_probe_script(), None);
    artifact_globs(&suffixes, &pkgver, &pkgrel, pkgext.trim())
}

/// Run one of the courted probe scripts with `bash -c <script> _ <path>`
/// (the `_` is `$0`, the PKGBUILD path is `$1` — the scripts source
/// `"$1"`). The pkgext probe takes no path. BOUNDED like [`exec_probe`].
///
/// The temp capture file name is UNIQUE PER CALL (pid + a process-wide
/// counter + the script length) AND created with O_EXCL|O_NOFOLLOW + 0600
/// (audit P1/security: the old `File::create` was predictable and
/// symlink-following — another local process could pre-create a symlink and
/// make the manager truncate an arbitrary victim-writable file). The
/// uniqueness also stops concurrent probes from truncating each other's
/// capture (the observed 201-byte mid-URL truncation).
fn run_probe(script: &str, file: Option<&std::path::Path>) -> String {
    static PROBE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    // O_EXCL (create_new) + 0600 (audit P1/security): the name is unique
    // per call, creation FAILS if anything (including a symlink) already
    // sits at the path, and the file is owner-only — a hostile local
    // process can neither predict a name to pre-create nor read the
    // capture.
    let out = std::env::temp_dir().join(format!(
        "km-probe-{}-{}-{}.out",
        std::process::id(),
        PROBE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        script.len()
    ));
    use std::os::unix::fs::OpenOptionsExt;
    let stdout = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&out)
    {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut cmd = std::process::Command::new("bash");
    cmd.arg("-c").arg(script).arg("km-probe");
    if let Some(f) = file {
        cmd.arg(f);
    }
    let mut child = match cmd
        .stdout(stdout)
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            let _ = std::fs::remove_file(&out);
            return String::new();
        }
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => break,
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    // THE .patch-filter killer: `echo "${source[@]}"` ends with a newline,
    // so the LAST source entry carries a trailing `\n` and
    // `entries.ends_with(".patch")` fails — the oracle's `utils::exec` pops
    // ONE trailing newline (`utils.cpp:99-117`) before the split+filter
    // (conf-window.cpp:465-466). Match that exactly, or the patches tab is
    // ALWAYS empty no matter how perfect the capture.
    let text = match text.strip_suffix('\n') {
        Some(t) => t.to_string(),
        None => text,
    };
    vlog!(
        "probe script {} ({} bytes) -> {}",
        script.len(),
        text.len(),
        text.chars().take(4000).collect::<String>()
    );
    text
}

/// Probe a systemd unit state (`systemctl is-enabled`/`is-active`): true
/// when the verb reports the ACTIVE/ENABLED state. The SCX apply/disable
/// decision branches on these (the scx.service conflict + the loader
/// enablement) — probed at the operation boundary, never cached (audit
/// P1-style: no stale machine facts).
fn systemctl_state(unit: &str, verb: &str) -> bool {
    std::process::Command::new("systemctl")
        .args([verb, unit])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            match verb {
                "is-active" => out.trim() == "active",
                _ => out.trim() == "enabled",
            }
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// The courted build-flow pipelines (review seams #5/#6)
// ---------------------------------------------------------------------------

/// Run one git command in a directory; success = exit 0.
fn git_run(cwd: Option<&std::path::Path>, args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    match cmd.status() {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

/// Execute the courted `git_cache_plan` (`prepare_git_repo`, utils.cpp:
/// 161-196) with the exact step order and abort semantics: create the parent
/// dir, enter it, wipe a stale non-git checkout, clone when missing, enter
/// the checkout, `checkout --force master`, `clean -fd`, `pull`.
///
/// Abort points (EnterParentDir/GitClone/EnterRepoDir failure stops the
/// whole sequence — the caller continues into the build like the oracle);
/// checkout/clean failure short-circuits the REMAINING refresh steps only
/// (the oracle's `||` chain, utils.cpp:191-195).
///
/// Deliberate divergence (D-004-ish): the oracle CHANGES the process cwd;
/// the candidate tracks the cwd per command (worker-thread local), so the
/// event-loop thread's cwd is untouched. The execve argv/cwd surface per
/// step is identical, which is what the git-cache VM court witnesses.
fn execute_git_cache_plan(repo: &std::path::Path, cache: &std::path::Path, url: &str) {
    use cachyos_kernel_manager_build::{git_cache_plan, GitCacheState, GitCacheStep};

    let state = GitCacheState {
        parent_dir_exists: cache.exists(),
        repo_exists: repo.exists(),
        repo_is_git: repo.join(".git").exists(),
    };
    let plan = git_cache_plan(&state, cache, repo, url);
    let mut cwd: Option<std::path::PathBuf> = None;
    let mut short_circuit = false;
    for step in &plan {
        vlog!("git-cache step: {step:?}");
        match step {
            GitCacheStep::CreateDirectories => {
                let _ = std::fs::create_dir_all(cache);
            }
            GitCacheStep::EnterParentDir => {
                cwd = Some(cache.to_path_buf());
                if !cache.is_dir() {
                    km_eprintln!("prepare_git_repo: cannot enter '{}'", cache.display());
                    return;
                }
            }
            GitCacheStep::RemoveNonGitRepo => {
                let _ = std::fs::remove_dir_all(repo);
            }
            GitCacheStep::GitClone { url, name } => {
                if !git_run(cwd.as_deref().or(Some(cache)), &["clone", url, name]) {
                    km_eprintln!("prepare_git_repo: 'git clone {url}' failed");
                    return; // aborts the whole sequence (utils.cpp:181-185)
                }
            }
            GitCacheStep::EnterRepoDir => {
                cwd = Some(repo.to_path_buf());
                if !repo.is_dir() {
                    km_eprintln!("prepare_git_repo: cannot enter '{}'", repo.display());
                    return;
                }
            }
            GitCacheStep::GitCheckoutForceMaster => {
                if !git_run(cwd.as_deref(), &["checkout", "--force", "master"]) {
                    short_circuit = true; // skips clean+pull (the || chain)
                }
            }
            GitCacheStep::GitCleanFd => {
                if !git_run(cwd.as_deref(), &["clean", "-fd"]) {
                    short_circuit = true; // skips pull
                }
            }
            GitCacheStep::GitPull => {
                let _ = git_run(cwd.as_deref(), &["pull"]);
            }
        }
        if short_circuit {
            break;
        }
    }
}

/// The oracle's `write_to_file` (`utils.cpp:80-92`): overwrite the PKGBUILD
/// (or any build-flow file).
fn write_pkgbuild(path: &std::path::Path, text: &str) -> bool {
    match std::fs::write(path, text) {
        Ok(()) => {
            vlog!("wrote {}", path.display());
            true
        }
        Err(e) => {
            km_eprintln!("[WRITE_TO_FILE] '{}' open failed: {}", path.display(), e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// The build's source-array + artifact probes
// ---------------------------------------------------------------------------

/// `reset_patches_data_tab` (`conf-window.cpp:458-473`): the current
/// variant's PKGBUILD source array (the options env spliced into the probe
/// script), filtered to entries ending with `.patch`. Empty when the
/// PKGBUILD is missing (no clone yet — the configure-prepare flow runs this
/// AFTER the git-cache plan).
fn probe_patch_entries(
    repo: &std::path::Path,
    variant: KernelVariant,
    options: &cachyos_kernel_manager_core::options::BuildOptions,
) -> Vec<String> {
    use cachyos_kernel_manager_build::{
        options_env_string, parse_source_array_probe_output, source_array_probe_script,
    };
    let pkgbuild = repo.join(variant.dir_name()).join("PKGBUILD");
    if !pkgbuild.exists() {
        vlog!("patches probe: no PKGBUILD at {}", pkgbuild.display());
        return Vec::new();
    }
    let env_string = options_env_string(options);
    let out = run_probe(&source_array_probe_script(&env_string), Some(&pkgbuild));
    let entries = parse_source_array_probe_output(&out);
    let patches: Vec<String> = entries
        .into_iter()
        .filter(|e| e.ends_with(".patch"))
        .collect();
    vlog!("patches probe: {} patch entries", patches.len());
    patches
}

// ---------------------------------------------------------------------------
// Background tasks (blocking work bridges into the Slint event loop)
// ---------------------------------------------------------------------------

/// A runtime action the backend dispatches (the slint event-loop analogue).
/// LAZY: a `Spawn` action runs only when dispatched — the tests never
/// dispatch, so no worker threads are created and no side effects happen.
#[derive(Default)]
pub enum Task {
    #[default]
    None,
    Batch(Vec<Task>),
    /// Spawn a background worker; on completion it delivers a `UiMessage`
    /// back into the event loop (the window weak + the shared app state).
    Spawn(Box<dyn FnOnce(slint::Weak<MainWindow>, Arc<Mutex<App>>) + Send>),
    /// Exit the event loop (the oracle's closeEvent).
    Exit,
}

/// Run a blocking closure on a worker thread and bridge the result into the
/// event loop. A PANIC in the closure is logged to stderr and delivered as
/// the task's FAILURE message (`make_fail`) — audit P1: the old code
/// substituted `A::default()`, which turned an ALPM panic into a
/// valid-looking EMPTY catalog ("discovery succeeded, zero kernels") and a
/// config panic into a silent empty result. A default value never stands in
/// for a panic; each task picks the fail-closed message for its own state
/// path (e.g. `BuildFinished { success: false }` for the build worker). The
/// alternative (a panic on the event loop) leaves the progress dialog up
/// forever with no message ever arriving (observed in the VM: a
/// discovery-data panic froze the app on "Initializing kernels...").
/// Deliver one UI message into the event loop from a worker thread (the
/// shared result bridge): update + sync + dispatch the resulting tasks. Used
/// by [`blocking`] for the final result and by the transaction worker for
/// its intermediate started signal.
fn deliver(app: &Arc<Mutex<App>>, msg: UiMessage) {
    let app = app.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Ok(mut state) = app.lock() else {
            return;
        };
        let task = update(&mut state, msg);
        state.sync_ui();
        drop(state);
        dispatch(task, &app);
    });
}

fn blocking<A, F>(f: F, make: fn(A) -> UiMessage, make_fail: fn(String) -> UiMessage) -> Task
where
    A: Send + 'static,
    F: FnOnce() -> A + Send + 'static,
{
    Task::Spawn(Box::new(move |_ui, app| {
        vlog!("background task spawn");
        std::thread::spawn(move || {
            let a = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
                Ok(a) => a,
                Err(p) => {
                    let message = panic_message(&p);
                    km_eprintln!("cachyos-kernel-manager: background task panicked: {message}");
                    // fail-CLOSED: the task's failure message, never a
                    // default-value stand-in (audit P1)
                    deliver(&app, make_fail(message));
                    return;
                }
            };
            vlog!("background task done");
            deliver(&app, make(a));
        });
    }))
}

/// The panic's message, as a String (a downcast of the payload + the
/// location fallback).
fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".to_string()
    }
}

/// Dispatch a task: spawn its workers (the shared app state rides along for
/// the result bridge).
fn dispatch(task: Task, app: &Arc<Mutex<App>>) {
    let ui = app.lock().map(|a| a.ui.clone()).unwrap_or_default();
    match task {
        Task::None => {}
        Task::Batch(tasks) => {
            for t in tasks {
                dispatch(t, app);
            }
        }
        Task::Spawn(f) => f(ui, app.clone()),
        Task::Exit => {
            vlog!("quit event loop");
            let _ = slint::quit_event_loop();
        }
    }
}

impl App {
    fn discovery_task(&self) -> Task {
        let flags = self.flags.clone();
        blocking(
            move || run_discovery(&flags),
            |result| match result {
                Ok(payload) => UiMessage::CatalogLoaded(Box::new(payload)),
                Err(message) => UiMessage::DiscoveryFailed(message),
            },
            |message| UiMessage::DiscoveryFailed(message),
        )
    }

    /// The transaction task: plan from the selection (AUR kernels included
    /// per the runtime `aur_enabled` flag), run the commit chain in the
    /// oracle's order via the courted `commit_commands` (AUR git-refresh +
    /// `makepkg -sicf` FIRST — kernel.cpp:289-294, aur_kernel.cpp — then
    /// `pacman -S --needed`, then `pacman -Rsn`), then the kernels-change
    /// check (`is_kernels_change_state`, km-window.cpp:150-166). The worker
    /// signals `TransactionStarted` when the commit run begins so the phase
    /// projection shows `TransactionRunning` for the real work.
    fn transaction_task(&self) -> Task {
        let selection = self.state.selection.clone();
        let kernels = self.kernels.clone();
        // the CACHED discovery snapshot (startup-static oracle facts are
        // correct here: root-on-ZFS + the chwd profiles are `static const`
        // in the oracle, kernel.cpp:43-52). The MUTABLE facts (local db,
        // module-family probes) are refreshed INSIDE the worker at the
        // transaction boundary (audit P1 TOCTOU — see below).
        let cached_hardware = self.hardware.clone();
        // the runtime AUR flag (the shipped build is OFF; the model must not
        // silently differ from the flag — review seam #3)
        let aur_enabled = self.flags.aur_enabled;

        Task::Spawn(Box::new(move |_ui, app| {
            vlog!("background task spawn (transaction)");
            std::thread::spawn(move || {
                // the commit run started: the phase projection enters
                // TransactionRunning (review seam #5)
                deliver(&app, UiMessage::TransactionStarted);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    use cachyos_kernel_manager_plan::commit_commands;
                    // audit P1 TOCTOU: the oracle re-queries the LIVE local
                    // database (nvidia-dkms/nvidia-open-dkms presence,
                    // companion removal) and the `pacman -Qqs` module-family
                    // probes at `Kernel::install()`/`remove()` time
                    // (kernel.cpp:105-130,143-161). A package operation
                    // between catalog display and Execute can change the
                    // NVIDIA/companion expansion — feeding the plan the
                    // CACHED discovery snapshot picks stale decisions. Only
                    // the startup-static oracle facts (root-on-ZFS, chwd)
                    // stay cached.
                    let hardware = refresh_mutable_hardware(&cached_hardware);
                    let mut plan = cachyos_kernel_manager_plan::TransactionPlan::from_selection(
                        &selection, &hardware, &kernels,
                    );
                    plan.aur_enabled = aur_enabled;
                    // the ORACLE's commit ordering (courted by
                    // commit_commands): AUR builds first, then the repo
                    // install, then the removal.
                    let commands = commit_commands(&plan);
                    let install_names: Vec<String> =
                        plan.install.iter().map(|a| a.package.clone()).collect();
                    let remove_names: Vec<String> =
                        plan.remove.iter().map(|a| a.package.clone()).collect();
                    vlog!(
                        "transaction plan: aur_enabled={} aur={:?} install {install_names:?} remove {remove_names:?}",
                        aur_enabled,
                        plan.aur_install
                    );
                    // the worker thread semantics (km-window.cpp:120-174):
                    // AUR builds, install, remove, commit, then the change
                    // check. The terminal-helper exit codes are LOGGED but
                    // the outcome is the changed-check — the ORACLE ignores
                    // the terminal exit code (runCmdTerminal, gap-008) and
                    // keys on the package state; the pacman error text is
                    // visible in the terminal itself.
                    // assigned by the alpm change-check branch (feature alpm)
                    #[allow(unused_mut)]
                    let mut failed: Option<String> = None;
                    for command in &commands {
                        match command {
                            cachyos_kernel_manager_exec::CommandPlan::GitRefresh { url, dir } => {
                                // the AUR git refresh (aur_kernel.cpp:32-36)
                                // — the SAME prepare_git_repo lifecycle as
                                // the repo cache: create dirs, enter parent,
                                // wipe a stale non-git checkout, clone,
                                // checkout --force master, clean -fd, pull.
                                // `~` is expanded via fix_path (audit P1: the
                                // old code handed the raw `~/.cache/...` to
                                // Path::new — creating a literal `~` dir —
                                // and never removed a stale non-git
                                // checkout, so git clone failed into a
                                // non-empty dir).
                                let target = cachyos_kernel_manager_exec::fix_path(dir);
                                let parent =
                                    target.parent().unwrap_or(target.as_path()).to_path_buf();
                                execute_git_cache_plan(&target, &parent, url);
                            }
                            cachyos_kernel_manager_exec::CommandPlan::BuildAurPackage { dir } => {
                                // makepkg runs NON-escalated IN the AUR dir
                                // (aur_kernel.cpp:53 — the oracle's
                                // prepare_git_repo left the process cwd
                                // inside the checkout, so runCmdTerminal's
                                // child inherits it; the candidate sets the
                                // terminal-helper's cwd explicitly). The dir
                                // is CARRIED by the plan — no reverse-scan of
                                // the command vector (audit P1: two AUR
                                // selections used to make every build run in
                                // the LAST refresh's directory).
                                let aur_dir = cachyos_kernel_manager_exec::fix_path(dir);
                                let _ = cachyos_kernel_manager_exec::run_cmd_terminal_at(
                                    &cachyos_kernel_manager_exec::makepkg_aur_argv().join(" "),
                                    cachyos_kernel_manager_exec::Escalate::None,
                                    &aur_dir.to_string_lossy(),
                                    &[],
                                );
                            }
                            cachyos_kernel_manager_exec::CommandPlan::InstallRepoPackages {
                                packages,
                                needed,
                            } => {
                                let cmd = cachyos_kernel_manager_exec::pacman_install_argv(
                                    packages, *needed,
                                )
                                .join(" ");
                                let rc = cachyos_kernel_manager_exec::run_cmd_terminal(
                                    &cmd,
                                    cachyos_kernel_manager_exec::Escalate::PkexecRootShell,
                                );
                                vlog!("transaction terminal exit: {rc} ({cmd})");
                            }
                            cachyos_kernel_manager_exec::CommandPlan::RemovePackages {
                                packages,
                            } => {
                                let cmd = cachyos_kernel_manager_exec::pacman_remove_argv(packages)
                                    .join(" ");
                                let rc = cachyos_kernel_manager_exec::run_cmd_terminal(
                                    &cmd,
                                    cachyos_kernel_manager_exec::Escalate::PkexecRootShell,
                                );
                                vlog!("transaction terminal exit: {rc} ({cmd})");
                            }
                            _ => {}
                        }
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
                        (changed, Some(message))
                    } else {
                        (changed, None)
                    }
                }));
                let (changed, failed) = match result {
                    Ok(r) => r,
                    Err(p) => {
                        km_eprintln!(
                            "cachyos-kernel-manager: transaction task panicked: {}",
                            panic_message(&p)
                        );
                        (false, Some("transaction worker panicked".to_string()))
                    }
                };
                vlog!("background task done (transaction)");
                deliver(
                    &app,
                    match failed {
                        Some(message) => UiMessage::TransactionFailed { message },
                        None => UiMessage::TransactionFinished { changed },
                    },
                );
            });
        }))
    }

    /// The Configure-window prepare flow (`on_configure`, km-window.cpp:
    /// 340-351): the courted `prepare_build_environment` (the git-cache
    /// plan of `utils.cpp:161-202`) + `reset_patches_data_tab` (the
    /// source-array probe for the current variant, `.patch`-filtered).
    /// Review seam #5: the OLD code ran an ad-hoc clone/pull; the production
    /// GUI must execute the courted `git_cache_plan` (create dirs, enter
    /// parent, remove the non-git checkout, clone, enter the checkout,
    /// `checkout --force master`, `clean -fd`, `pull`).
    fn configure_task(&mut self) -> Task {
        // the operation epoch: captured at dispatch so the prepare result is
        // tied to THIS prepare's variant snapshot (audit P1 — a variant/
        // mutation that lands while the prepare is in flight bumps the
        // epoch and makes this result stale).
        self.patch_epoch += 1;
        let generation = self.patch_epoch;
        let home = self.flags.home.clone();
        let options = self.state.build_options.clone();
        let variant = self.configure.variant;
        blocking(
            move || {
                let cache = cachyos_kernel_manager_platform::cache_root(&home);
                let repo = cachyos_kernel_manager_platform::pkgbuilds_dir(&home);
                execute_git_cache_plan(
                    &repo,
                    &cache,
                    cachyos_kernel_manager_platform::LINUX_CACHYOS_GIT_URL,
                );
                // reset_patches_data_tab: the current variant's PKGBUILD
                // source array (options env spliced), filtered to .patch
                let patches = probe_patch_entries(&repo, variant, &options);
                (generation, patches)
            },
            |(generation, patches)| UiMessage::ConfigurePrepared {
                generation,
                patches,
            },
            // a prepare panic FAILS the flow: error dialog + the Configure
            // state closes (never an empty patch list masquerading as a
            // successful prepare — audit P1)
            |message| UiMessage::ConfigureFailed(message),
        )
    }

    /// The `reset_patches_data_tab` re-probe after a variant/lto/nvidia-open
    /// change (the oracle calls it from those handlers: conf-window.cpp:601,
    /// 603-605, 407-419). Async: the probe needs the cloned PKGBUILD, so it
    /// must not block the event loop.
    fn refresh_patches_task(&self, generation: u64) -> Task {
        let home = self.flags.home.clone();
        let variant = self.configure.variant;
        let options = self.state.build_options.clone();
        blocking(
            move || {
                let repo = cachyos_kernel_manager_platform::pkgbuilds_dir(&home);
                let patches = probe_patch_entries(&repo, variant, &options);
                (generation, patches)
            },
            |(generation, patches)| UiMessage::PatchesRefreshed {
                generation,
                patches,
            },
            // a refresh panic: plain error dialog, the current patch list
            // stays untouched (a refresh result is a nicety, never worth
            // clobbering the user's edits — audit P1)
            |message| UiMessage::TaskFailed(message),
        )
    }

    /// The Build button's worker (`on_execute`, conf-window.cpp:696-735):
    /// the FULL courted pipeline — `prepare_build_environment` (git-cache
    /// plan), `restore_clean_environment` (options env application), the
    /// source-array probe, the patch source-array mutation, the custom-name
    /// mutation, then the build through the courted terminal-helper, with
    /// success defined by `.done-status` presence. Review seam #6: the OLD
    /// code bypassed the model with a raw `bash -lc` and never mutated the
    /// PKGBUILD. `epoch` is the operation generation established at
    /// dispatch (audit P0): the worker aborts if a Configure cancel/close
    /// bumped it before the terminal spawn OR after the child was stored.
    fn build_task(&self, epoch: u64) -> Task {
        use cachyos_kernel_manager_build::{
            insert_custom_pkgbase, insert_patch_source_array, options_env_string,
            parse_source_array_probe_output, source_array_probe_script,
        };
        use cachyos_kernel_manager_exec::BuildFlowPlan;

        let home = self.flags.home.clone();
        let options = self.state.build_options.clone();
        let variant = self.configure.variant;
        let patches = self.configure.patches.clone();
        let custom_name = self.configure.custom_name.clone();
        // the owned cancellation contract (the Configure window's close/cancel
        // terminates the in-flight build — D-008, an explicit correction)
        let epoch_ref = self.build_epoch.clone();
        let proc_slot = self.build_proc.clone();
        blocking(
            move || {
                let cache = cachyos_kernel_manager_platform::cache_root(&home);
                let repo = cachyos_kernel_manager_platform::pkgbuilds_dir(&home);
                // 1. prepare_build_environment — the courted git-cache plan
                //    (the oracle runs it here too, before the env restore).
                execute_git_cache_plan(
                    &repo,
                    &cache,
                    cachyos_kernel_manager_platform::LINUX_CACHYOS_GIT_URL,
                );
                // 2. the options env string (get_all_set_values).
                let env_string = options_env_string(&options);
                // 3. the build-option environment as PER-CHILD assigns
                //    (audit P1/security: the OLD restore_clean_environment
                //    called std::env::set_var/remove_var from a worker
                //    thread while the Slint event loop + D-Bus threads run —
                //    unsound on multithreaded programs. The env now travels
                //    inside the probe script text (it is spliced verbatim,
                //    conf-window.cpp:204-216) and is applied to the
                //    terminal-helper child via Command::envs below; the
                //    manager process env is never mutated.)
                let assigns = cachyos_kernel_manager_build::env_assignments(&env_string);
                // 4. the source-array probe (the script embeds the env).
                let variant_dir = repo.join(variant.dir_name());
                let pkgbuild_path = variant_dir.join("PKGBUILD");
                let src = run_probe(
                    &source_array_probe_script(&env_string),
                    Some(&pkgbuild_path),
                );
                let orig_entries = parse_source_array_probe_output(&src);
                // 5. the patch source-array mutation + the custom-name
                //    mutation (both write the PKGBUILD back; a failed
                //    write aborts like the oracle's insert_status check).
                //    D-003 SECURITY_CORRECTION: a hostile custom name or
                //    patch entry (quote/newline/$/backtick/backslash) would
                //    escape the splice and become PKGBUILD code that makepkg
                //    EVALUATES — reject it at the boundary. The custom-name
                //    grammar explicitly PERMITS the oracle's default
                //    `$pkgbase-custom` and the `$pkgbase` sentinel (an
                //    untouched Configure -> Build must pass); arbitrary
                //    `${...}`/`$()`/`$name` expansions are rejected.
                if !cachyos_kernel_manager_build::splice_safe_custom_name(&custom_name) {
                    km_eprintln!(
                        "cachyos-kernel-manager: custom package name rejected (unsafe splice input) — D-003: {custom_name:?}"
                    );
                    return false;
                }
                for p in &patches {
                    if let Some(i) = cachyos_kernel_manager_build::splice_unsafe_index(p) {
                        km_eprintln!(
                            "cachyos-kernel-manager: patch entry rejected (unsafe byte at {i}): {p:?} — D-003"
                        );
                        return false;
                    }
                }
                let pkgbuild_text = std::fs::read_to_string(&pkgbuild_path).unwrap_or_default();
                let mutated = insert_patch_source_array(&pkgbuild_text, &orig_entries, &patches);
                if !write_pkgbuild(&pkgbuild_path, &mutated) {
                    km_eprintln!(
                        "cachyos-kernel-manager: Failed to insert new source array into pkgbuild"
                    );
                    return false;
                }
                let text2 = std::fs::read_to_string(&pkgbuild_path).unwrap_or_default();
                let mutated2 = insert_custom_pkgbase(&text2, &custom_name);
                if !write_pkgbuild(&pkgbuild_path, &mutated2) {
                    km_eprintln!("cachyos-kernel-manager: Failed to set custom name in pkgbuild");
                    return false;
                }
                // 6. run the build through the courted terminal-helper IN the
                //    variant directory (the oracle's
                //    `QProcess::setWorkingDirectory(working_path)`,
                //    conf-window.cpp:733 — makepkg must find the PKGBUILD
                //    there and `.done-status` lands in the variant dir). The
                //    child is OWned by this task so the Configure close/cancel
                //    can terminate it (the oracle destroys its QProcess).
                let plan = BuildFlowPlan::render(variant, repo.to_str().unwrap_or_default(), &[]);
                // the operation-generation check BEFORE the spawn: a cancel
                // that landed before the worker started (or during the
                // git-cache/mutation steps) aborts here (audit P0 — the old
                // boolean was cleared by the worker itself, so a cancel
                // before start was silently overwritten)
                if epoch != epoch_ref.load(Ordering::Relaxed) {
                    vlog!("build aborted by Configure cancel before the terminal launch");
                    return false;
                }
                vlog!(
                    "build: cwd={} cmd={:?} done={}",
                    plan.working_path,
                    plan.terminal_argv,
                    plan.done_status
                );
                let child = match cachyos_kernel_manager_exec::spawn_cmd_terminal(
                    &plan.build_command,
                    cachyos_kernel_manager_exec::Escalate::None,
                    &plan.working_path,
                    &assigns,
                ) {
                    Ok(c) => c,
                    Err(_) => {
                        km_eprintln!("cachyos-kernel-manager: terminal-helper failed to start");
                        return false;
                    }
                };
                // the post-spawn check: a cancel that landed BETWEEN the
                // pre-check and the child landing in the slot must still
                // abort — kill the just-spawned child and fail (audit P0)
                if epoch != epoch_ref.load(Ordering::Relaxed) {
                    vlog!("build aborted by Configure cancel right after the terminal spawn");
                    let mut child = child;
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                *proc_slot.lock().unwrap() = Some(child);
                // wait for the helper; the cancel path takes + kills the
                // child, which makes this poll see an empty slot and exit
                loop {
                    let finished = proc_slot
                        .lock()
                        .unwrap()
                        .as_mut()
                        .map_or(true, |c| c.try_wait().ok().flatten().is_some());
                    if finished {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                *proc_slot.lock().unwrap() = None;
                // 7. success = `.done-status` PRESENT, never the exit code.
                //    The marker is REMOVED at the exact oracle transition
                //    point (finished_proc:384-389 checks existence, then
                //    deletes it BEFORE the install question) — a stale
                //    marker must never misclassify a later failed build.
                let ok = std::path::Path::new(&plan.done_status).exists();
                let _ = std::fs::remove_file(&plan.done_status);
                // a cancelled build is the FAILURE branch (D-008 — the
                // terminated build never completes)
                let ok = ok && epoch == epoch_ref.load(Ordering::Relaxed);
                vlog!("build finished, done-status present: {ok} (marker removed)");
                ok
            },
            |success| UiMessage::BuildFinished { success },
            // a PANIC in the build worker is the FAILURE branch — fail-
            // closed, immediately retryable (audit P1: the old default `false`
            // happened to be correct here, but the principle is now explicit)
            |_| UiMessage::BuildFinished { success: false },
        )
    }

    /// The artifact install task — the REAL `sudo pacman -U <globs>`
    /// (review seam #8: the old code executed `true`). The globs come from
    /// the courted pkgfuncs probe on the built PKGBUILD (the artifact-glob
    /// model), like the oracle's `finished_proc` install path. The child is
    /// OWNED by the same build-proc slot so the Configure window's close/
    /// cancel can terminate the install too (audit P0: it used to be
    /// unowned — the cancellation only covered the makepkg phase). `epoch`
    /// is the current operation generation: a cancel before the spawn
    /// aborts the install.
    fn artifacts_task(&self, epoch: u64) -> Task {
        let home = self.flags.home.clone();
        let variant_dir = self.configure.variant.dir_name().to_string();
        let epoch_ref = self.build_epoch.clone();
        let proc_slot = self.build_proc.clone();
        blocking(
            move || {
                if epoch != epoch_ref.load(Ordering::Relaxed) {
                    vlog!(
                        "artifact install aborted by Configure cancel before the terminal launch"
                    );
                    return;
                }
                let variant_dir =
                    cachyos_kernel_manager_platform::pkgbuilds_dir(&home).join(&variant_dir);
                let pkgbuild = variant_dir.join("PKGBUILD");
                let globs = build_artifact_globs(&pkgbuild);
                if globs.is_empty() {
                    km_eprintln!(
                        "cachyos-kernel-manager: artifact install skipped (no package globs from {})",
                        pkgbuild.display()
                    );
                } else {
                    // the oracle runs the install through ordinary
                    // run_cmd_async with the working directory set to the
                    // variant dir (m_build_conf_path) — the terminal is NOT
                    // launched through the pkexec root-shell path; `sudo`
                    // stays INSIDE the command (finished_proc:394-401).
                    let cmd = format!("sudo pacman -U {}", globs.join(" "));
                    let child = match cachyos_kernel_manager_exec::spawn_cmd_terminal(
                        &cmd,
                        cachyos_kernel_manager_exec::Escalate::None,
                        &variant_dir.to_string_lossy(),
                        &[],
                    ) {
                        Ok(c) => c,
                        Err(_) => {
                            km_eprintln!("cachyos-kernel-manager: terminal-helper failed to start");
                            return;
                        }
                    };
                    *proc_slot.lock().unwrap() = Some(child);
                    // wait; the cancel path takes + kills the child, which
                    // makes this poll see an empty slot and exit
                    loop {
                        let finished = proc_slot
                            .lock()
                            .unwrap()
                            .as_mut()
                            .map_or(true, |c| c.try_wait().ok().flatten().is_some());
                        if finished {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    *proc_slot.lock().unwrap() = None;
                }
            },
            |_| UiMessage::ArtifactsInstalled,
            // a panic while the install flow runs: the oracle treats the
            // install as fire-and-forget (finished_proc spawns run_cmd_async
            // and the terminal shows pacman's outcome), so the flow is OVER
            // — return to Idle, never a soft-lock in Installing. The panic
            // text is in the km log (audit P1).
            |_| UiMessage::ArtifactsInstalled,
        )
    }

    /// The sched-ext window init: config init + the loader supported list
    /// (the REAL D-Bus with the `scx-dbus` feature; the frozen list
    /// otherwise) + the sysfs current-scheduler readback, through the
    /// courted `window_init` trace.
    fn scx_init_task(&self) -> Task {
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
                    config: config.clone(),
                    current_scheduler_label: current,
                });
                // the model carries the ACTUALLY LOADED config (audit P0:
                // the apply plan's args-vs-mode decision + the persisted
                // defaults must come from /etc/scx_loader.toml, never a
                // reconstructed default_config())
                ScxWindowModel::from_init_steps(&steps, config_path, "Auto", config)
            },
            |model| UiMessage::ScxInit(Box::new(model)),
            // an scx-init panic: error dialog instead of a garbage default
            // model opening an empty window (audit P1)
            |message| UiMessage::TaskFailed(message),
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

/// The update function: one UI message -> the courted semantic transition
/// + the UI-side model updates + the presentation sync.
pub fn update(app: &mut App, message: UiMessage) -> Task {
    vlog!("update: {message:?}");
    match message {
        UiMessage::Semantic(m) => {
            // The Configure window's Cancel/Close OWNS the in-flight build:
            // closing the window terminates the build process (the oracle
            // destroys its `QProcess m_cmd` member — conf-window.cpp:688-690,
            // courted by `configure_trace`). Signal + kill the worker's
            // terminal-helper so it reports the oracle's FAILURE branch
            // (no `.done-status` → BuildFinished{false}).
            if matches!(
                m,
                Message::ConfigurationCancelRequested | Message::ConfigurationCloseRequested
            ) && app.state.build.in_flight()
            {
                cancel_build_process(app);
            }
            app.on_semantic(m)
        }
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
            // a fresh catalog seeds the DISPLAYED order: the oracle's
            // `init_kernels` clears the tree and re-adds the items (the
            // current sort column's auto-sort re-orders them; the stable
            // tie-break base is the new discovery order).
            app.sorted_rows = payload.rows.clone();
            app.recompute_sort();
            let task = app.on_event(AppEvent::DiscoveryFinished);
            // a GENUINE empty catalog raises the oracle's critical dialog
            // (`init_kernels`, km-window.cpp:228-230: `if (m_kernels.empty())`
            // → "No kernels found!...") — the old code treated an empty
            // result as a normal Ready with no dialog (audit P1: an ALPM
            // panic used to masquerade as this exact state, so the two must
            // be distinguishable — a failure shows the init dialog via
            // DiscoveryFailed, an empty probe shows this one).
            if payload.rows.is_empty() {
                app.state.dialogs = DialogsState::Error {
                    message: app.tr(tr_ctx::MAIN, strings::dialogs::NO_KERNELS),
                };
            }
            task
        }
        UiMessage::DiscoveryFailed(message) => {
            // audit P1 fail-closed: an ALPM init failure or a discovery
            // panic is a TASK FAILURE — the oracle's "Failed to initialize
            // alpm handle (%1)" dialog (km-window.cpp:144), never a
            // successful empty catalog. The app readies with an empty
            // catalog (the OK stays disabled), exactly like the oracle's
            // null-handle startup behavior, but the dialog says WHY.
            app.rows.clear();
            app.kernels.clear();
            app.installed.clear();
            app.hardware = HardwareProfile::default();
            app.state.selection.rows.clear();
            app.state.dialogs = DialogsState::Error {
                message: app
                    .tr(tr_ctx::MAIN, strings::dialogs::FAILED_ALPM_INIT)
                    .replace("%1", &message),
            };
            app.on_event(AppEvent::DiscoveryFinished)
        }
        UiMessage::ConfigureFailed(message) => {
            // a configure-prepare panic: show the error and close the
            // Configure flow (state -> Closed) — never an empty patch list
            // masquerading as a successful prepare (audit P1).
            app.state.dialogs = DialogsState::Error { message };
            app.on_event(AppEvent::ConfigurationCancelRequested)
        }
        UiMessage::TaskFailed(message) => {
            // a generic background-task panic: a plain error dialog. The
            // task's own state path is untouched (the caller picked the
            // fail-closed message where one exists); the panic text is also
            // in the km log.
            app.state.dialogs = DialogsState::Error { message };
            Task::None
        }
        UiMessage::TransactionStarted => app.on_event(AppEvent::TransactionStarted),
        UiMessage::TransactionFinished { changed } => {
            app.on_event(AppEvent::TransactionFinished { changed })
        }
        UiMessage::TransactionFailed { message } => {
            app.on_event(AppEvent::TransactionFailed { message })
        }
        UiMessage::ConfigError(message) => {
            // a PLAIN error dialog — config I/O is not a pacman transaction,
            // so it must never drive the transaction state machine (routing
            // it through TransactionFailed soft-locked the OK button)
            app.state.dialogs = DialogsState::Error { message };
            Task::None
        }
        UiMessage::ConfigurePrepared {
            generation,
            patches,
        } => {
            // reset_patches_data_tab (km-window.cpp:349) + the state
            // transition into Editing. A STALE prepare (a variant/mutation
            // bumped the epoch while this worker was in flight — a newer
            // probe is already dispatched) is discarded: the newer result
            // owns the list (audit P1 — the old handler blindly replaced
            // the list with whichever worker finished last).
            if generation != app.patch_epoch {
                vlog!(
                    "patches: stale ConfigurePrepared (gen {generation} != {}) discarded",
                    app.patch_epoch
                );
                return Task::None;
            }
            app.configure.reset_patches(&patches);
            app.sync_build_options();
            app.on_event(AppEvent::ConfigurePrepared)
        }
        UiMessage::PatchesRefreshed {
            generation,
            patches,
        } => {
            // a variant/lto/nvidia-open change re-probed the source array.
            // Only the LATEST generation owns the list: a rapid A→B change
            // can finish B first and A second, and a user patch mutation
            // bumps the epoch so an older in-flight probe can never erase it
            // (audit P1 — the old result carried no fingerprint and blindly
            // replaced the list).
            if generation != app.patch_epoch {
                vlog!(
                    "patches: stale PatchesRefreshed (gen {generation} != {}) discarded",
                    app.patch_epoch
                );
                return Task::None;
            }
            app.configure.reset_patches(&patches);
            Task::None
        }
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
            Task::None
        }
        UiMessage::ScxApplied { ok } => {
            if !ok {
                if let Some(scx) = &app.scx {
                    if let Some(critical) = scx.apply_decision(false) {
                        app.state.dialogs = DialogsState::Error { message: critical };
                    }
                }
            }
            Task::None
        }
        UiMessage::ScxDisabled { ok } => {
            if !ok {
                if let Some(scx) = &app.scx {
                    if let Some(critical) = scx.disable_critical(false) {
                        app.state.dialogs = DialogsState::Error { message: critical };
                    }
                }
            }
            Task::None
        }
        UiMessage::DialogDismissed => {
            app.state.dialogs = DialogsState::None;
            // a FAILED transaction's error dialog was acknowledged: the
            // worker is done and the OK button re-enables (the oracle's
            // m_running releases after the worker's finished path — review
            // seam #1: the transaction used to park in Failed forever,
            // soft-locking Execute for the process lifetime).
            if app.state.transaction == TransactionState::Failed {
                app.on_event(AppEvent::TransactionErrorAcknowledged)
            } else {
                Task::None
            }
        }
        UiMessage::InstallQuestion(yes) => {
            app.state.dialogs = DialogsState::None;
            if yes {
                app.on_event(AppEvent::InstallArtifactsRequested)
            } else {
                // answering No ends the build flow: back to Idle, the Build
                // button is immediately retryable (m_running was cleared at
                // the start of finished_proc — audit P0)
                app.on_event(AppEvent::InstallDeclined)
            }
        }
        UiMessage::CustomNameChanged(value) => {
            app.configure.custom_name = value;
            app.sync_build_options();
            Task::None
        }
        UiMessage::ScxFlagsChanged(value) => {
            if let Some(scx) = &mut app.scx {
                scx.flags = value;
            }
            Task::None
        }
        UiMessage::ConfigLoaded(config) => {
            let task = app.on_semantic(Message::ConfigLoaded { config: *config });
            app.sync_build_options();
            // a loaded config can change LTO / builtin-nvidia-open / the
            // variant — all source-array-affecting — so the oracle's
            // `reset_patches_data_tab` must re-run for the new settings
            // (audit P1: the old path never re-probed after a config load)
            app.patch_epoch += 1;
            Task::Batch(vec![task, app.refresh_patches_task(app.patch_epoch)])
        }
        UiMessage::Exit => Task::Exit,
        UiMessage::SortRequested { column } => {
            if app.sort_column == column {
                // clicking the CURRENT sort column toggles the order
                app.sort_ascending = !app.sort_ascending;
            } else {
                // clicking a NEW column RESETS to ascending — the oracle's
                // QTreeWidget header click (`_q_headerClicked`,
                // sortingEnabled, km-window.ui) sets a new sort indicator
                // column with the CURRENT order only when the column is
                // unchanged; a different section starts ascending. Witnessed
                // by the ui/gui-drive court 2026-08-23: the frozen Qt tree's
                // PkgName/Version/Category orders are all ASCENDING after the
                // Choose click, with the previous column's order as the
                // stable tie-break. The old code kept the current order,
                // which produced the descending orders the oracle never
                // shows.
                app.sort_column = column;
                app.sort_ascending = true;
            }
            app.recompute_sort();
            Task::None
        }
        UiMessage::ConfTabClicked(tab) => {
            app.conf_tab = tab;
            Task::None
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
            app.sync_build_options();
            // the oracle re-probes the patches tab when the builtin-nvidia
            // checkbox changes (connect_all_checkboxes, conf-window.cpp:407-419)
            if check == ConfCheck::NvidiaOpen {
                app.patch_epoch += 1;
                app.refresh_patches_task(app.patch_epoch)
            } else {
                Task::None
            }
        }
        UiMessage::PatchOp(op) => {
            match op {
                ConfPatchOp::MoveUp(i) => app.configure.move_up(i),
                ConfPatchOp::MoveDown(i) => app.configure.move_down(i),
                ConfPatchOp::Remove(i) => app.configure.remove_patch(i),
            }
            // a list mutation bumps the epoch: an in-flight refresh that
            // completes AFTER the edit must not erase it (audit P1)
            app.patch_epoch += 1;
            Task::None
        }
        UiMessage::VariantPicked(variant) => {
            let task = app.on_semantic(Message::VariantChanged { variant });
            app.sync_build_options();
            // the oracle re-probes the patches tab on a variant switch
            // (conf-window.cpp:601). Bump FIRST: the refresh belongs to the
            // NEW variant, and any older in-flight probe is now stale.
            app.patch_epoch += 1;
            Task::Batch(vec![task, app.refresh_patches_task(app.patch_epoch)])
        }
        UiMessage::LtoPicked(lto) => {
            app.configure.switch.lto_selected = lto;
            app.sync_build_options();
            // the oracle re-probes the patches tab on an lto change
            // (conf-window.cpp:603-605)
            app.patch_epoch += 1;
            app.refresh_patches_task(app.patch_epoch)
        }
        UiMessage::PreemptPicked(preempt) => {
            app.configure.switch.preempt_selected = preempt;
            app.sync_build_options();
            Task::None
        }
        UiMessage::HzPicked(hz) => {
            app.configure.switch.hz_selected = hz;
            app.sync_build_options();
            Task::None
        }
        UiMessage::TicklessPicked(tickless) => {
            app.configure.tickless = tickless;
            app.sync_build_options();
            Task::None
        }
        UiMessage::HugepagePicked(hugepage) => {
            app.configure.hugepage = hugepage;
            app.sync_build_options();
            Task::None
        }
        UiMessage::CpuOptPicked(cpu_opt) => {
            app.configure.cpu_opt = cpu_opt;
            app.sync_build_options();
            Task::None
        }
        UiMessage::ScxWindowClosed => app.on_event(AppEvent::ScxWindowClosed),
        UiMessage::ScxApply => {
            if let Some(scx) = &app.scx {
                // FAIL-CLOSED parse: an unknown scheduler must never
                // silently become bpfland (audit P0 — the old runtime
                // used parse().unwrap_or(Bpfland))
                let parsed = scx
                    .scheduler
                    .parse::<cachyos_kernel_manager_scx::config::SupportedSched>();
                if let Err(e) = parsed {
                    app.state.dialogs = DialogsState::Error { message: e };
                    return Task::None;
                }
                let mode = scx.scx_mode();
                let flags = scx.flags.trim().to_string();
                let scheduler = scx.scheduler.clone();
                // the ACTUALLY LOADED config (audit P0: never a
                // reconstructed default_config())
                let config = scx.config.clone();
                let config_path = scx.config_path.clone();
                blocking(
                    move || {
                        #[cfg(feature = "scx-dbus")]
                        {
                            use cachyos_kernel_manager_scx::apply::{
                                apply_plan, execute_apply, ApplyInput, DbResult,
                            };
                            // the REAL service states the apply decision
                            // branches on (scx.service conflict, loader
                            // enablement)
                            let input = ApplyInput {
                                scx_name: scheduler,
                                scx_mode: mode,
                                extra_flags: flags,
                                config,
                                scx_service_enabled: systemctl_state("scx", "is-enabled"),
                                scx_service_active: systemctl_state("scx", "is-active"),
                                scx_loader_service_enabled: systemctl_state(
                                    "scx_loader",
                                    "is-enabled",
                                ),
                                config_path,
                                db_result: DbResult::Ok,
                                db_error: String::new(),
                            };
                            // ONE typed plan from the courted model; the
                            // executor interprets it (audit P0)
                            let plan = apply_plan(&input);
                            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
                            rt.block_on(async {
                                let connection = zbus::Connection::system().await;
                                match connection {
                                    Ok(conn) => execute_apply(&plan, &conn).await,
                                    Err(_) => false,
                                }
                            })
                        }
                        #[cfg(not(feature = "scx-dbus"))]
                        {
                            let _ = (mode, flags, scheduler, config, config_path);
                            true // offline: nothing to apply against
                        }
                    },
                    |ok| UiMessage::ScxApplied { ok },
                    // a panic while applying: fail-CLOSED — the existing
                    // handler raises the apply-failure critical dialog (audit
                    // P1: the old `true` default claimed success)
                    |_| UiMessage::ScxApplied { ok: false },
                )
            } else {
                Task::None
            }
        }
        UiMessage::ScxDisable => {
            if let Some(scx) = &app.scx {
                let config = scx.config.clone();
                let config_path = scx.config_path.clone();
                blocking(
                    move || {
                        #[cfg(feature = "scx-dbus")]
                        {
                            use cachyos_kernel_manager_scx::apply::{disable_plan, execute_apply};
                            // the courted disable plan: stop_scheduler + clear
                            // default_sched + persist (audit P0 — the old
                            // runtime only called stop_scheduler, losing the
                            // service-state and reboot parity)
                            let plan = disable_plan(&config, &config_path);
                            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
                            rt.block_on(async {
                                let connection = zbus::Connection::system().await;
                                match connection {
                                    Ok(conn) => execute_apply(&plan, &conn).await,
                                    Err(_) => false,
                                }
                            })
                        }
                        #[cfg(not(feature = "scx-dbus"))]
                        {
                            let _ = (config, config_path);
                            true
                        }
                    },
                    |ok| UiMessage::ScxDisabled { ok },
                    |_| UiMessage::ScxDisabled { ok: false },
                )
            } else {
                Task::None
            }
        }
        UiMessage::ScxSchedulerPicked(scheduler) => {
            if let Some(scx) = &mut app.scx {
                // the LOADED config, not a reconstructed default (audit P0)
                let config = scx.config.clone();
                scx.on_sched_changed(&scheduler, &config);
            }
            Task::None
        }
        UiMessage::ScxProfilePicked(profile) => {
            if let Some(scx) = &mut app.scx {
                let config = scx.config.clone();
                scx.on_profile_changed(&profile, &config);
            }
            Task::None
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
            Task::None
        }
        UiMessage::PathDialogChanged(value) => {
            if let Some(dialog) = &mut app.path_dialog {
                dialog.value = value;
            }
            Task::None
        }
        UiMessage::PathDialogDismissed => {
            app.path_dialog = None;
            Task::None
        }
        UiMessage::PathDialogSubmitted => {
            let Some(dialog) = app.path_dialog.take() else {
                return Task::None;
            };
            match dialog.on_submit {
                PathDialogKind::LoadConfig => {
                    let path = dialog.value;
                    // Option<Result<..>>: the outer None = the task panicked
                    // (the blocking fallback), the inner Result = the load
                    blocking(
                        move || {
                            Some(
                                KernelManagerConfig::load(std::path::Path::new(&path))
                                    .map_err(|e| e.to_string()),
                            )
                        },
                        |result| match result {
                            Some(Ok(config)) => UiMessage::ConfigLoaded(Box::new(config)),
                            Some(Err(message)) => UiMessage::ConfigError(message),
                            None => UiMessage::ConfigError(
                                "Failed to load config options from file".into(),
                            ),
                        },
                        // a panic = a plain config-error dialog (the old
                        // outer-None fallback covered it; explicit now — audit
                        // P1)
                        |message| UiMessage::ConfigError(message),
                    )
                }
                PathDialogKind::SaveConfig => {
                    let path = dialog.value;
                    let config = app.configure.to_config();
                    blocking(
                        move || {
                            Some(
                                config
                                    .save(std::path::Path::new(&path))
                                    .map_err(|e| e.to_string()),
                            )
                        },
                        |result| match result {
                            Some(Ok(())) => UiMessage::DialogDismissed,
                            Some(Err(e)) => UiMessage::ConfigError(format!(
                                "Failed to save config options to file: {e}"
                            )),
                            None => UiMessage::ConfigError(
                                "Failed to save config options to file".into(),
                            ),
                        },
                        |message| UiMessage::ConfigError(message),
                    )
                }
                PathDialogKind::AddRemotePatch => {
                    // the oracle's remote input returns WITHOUT changing
                    // state when cancelled or empty (`!is_confirmed ||
                    // patch_url_text.isEmpty()`, conf-window.cpp:643-646)
                    if dialog.value.trim().is_empty() {
                        return Task::None;
                    }
                    app.configure.add_remote_patch(dialog.value);
                    // a list mutation bumps the epoch: an in-flight refresh
                    // completing after must not erase the new entry (audit P1)
                    app.patch_epoch += 1;
                    Task::None
                }
                PathDialogKind::AddLocalPatch => {
                    // the oracle's file picker returns without changing
                    // state when nothing was selected (files.isEmpty(),
                    // conf-window.cpp:620-622); an empty submission is the
                    // inline dialog's cancel-equivalent
                    if dialog.value.trim().is_empty() {
                        return Task::None;
                    }
                    app.configure.add_local_patches(&[dialog.value]);
                    app.patch_epoch += 1;
                    Task::None
                }
            }
        }
    }
}

/// Terminate the in-flight Configure build/install (D-008 — an explicit
/// INTENTIONAL_CORRECTION: the frozen Qt window does NOT destroy a process
/// on close — no WA_DeleteOnClose, close() just hides — so the oracle lets
/// the build run to completion; the candidate terminates it for
/// availability. The VM oracle court proves the difference). Bumps the
/// operation generation (invalidating the in-flight worker) and kills the
/// owned terminal-helper child; the worker's wait returns and it reports
/// the FAILURE branch (`BuildFinished { success: false }`).
fn cancel_build_process(app: &mut App) {
    // bump the generation FIRST (the worker checks it before + after the
    // spawn — a cancel that lands in either window aborts the build)
    app.build_epoch.fetch_add(1, Ordering::Relaxed);
    if let Some(mut child) = app.build_proc.lock().unwrap().take() {
        vlog!("build cancelled: terminating the terminal-helper child");
        let _ = child.kill();
        let _ = child.wait();
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// run (the Slint entry point)
// ---------------------------------------------------------------------------

/// Launch the Slint application: build the windows, wire the callbacks to
/// the courted semantic surface, and run the event loop. Slint's default
/// renderer is FemtoVG (GPU-accelerated, REQUIRES OpenGL); the GPU-less
/// path is the winit-SOFTWARE renderer — the VM courts + CI set
/// `SLINT_BACKEND=winit-software` explicitly so the GUI runs in qemu
/// without any GL/Vulkan setup (unlike the wgpu-based iced port).
pub fn run(flags: Flags) -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    // The initial window sizes: a `width` binding on a Window makes it
    // FIXED-size in Slint 1.17 and `preferred-width` cannot drive the initial
    // size (the WindowItem layout info is always zero — the winit adapter
    // sizes from the width PROPERTY). set_size() sets a writable size: the
    // window opens at the oracle-like size and the user's resizes stick.
    ui.window().set_size(slint::LogicalSize::new(1000., 700.));
    // The CachyOS green accent: set now (backend exists) and re-applied on
    // every UI sync (the XDG settings watcher can override it later).
    set_cachyos_accent();
    // KWin renders the titlebar icon from the app's desktop entry matched via
    // the xdg app id — the .slint `icon` property only reaches X11's
    // _NET_WM_ICON. WITHOUT this the titlebar shows the generic yellow
    // "wayland" icon even though the taskbar is correct. NOTE: this must run
    // AFTER the first window creation (the backend + global context exist
    // then) and BEFORE any window is shown (ensure_window reads it).
    let _ = slint::set_xdg_app_id("org.cachyos.KernelManager");
    let configure = ConfigureWindow::new()?;
    // 900x900: the options page (~750px of checkboxes/combos) fits without
    // scrolling at this height; smaller windows scroll (the min stays 640x560).
    configure
        .window()
        .set_size(slint::LogicalSize::new(900., 900.));
    let scx_window = SchedExtWindow::new()?;
    // compact: the dialog's content is short (running label, two combos, the
    // flags input, three buttons) — a tall default just looks stretched. The
    // window minimum equals this size: it can grow, never shrink.
    scx_window
        .window()
        .set_size(slint::LogicalSize::new(480., 320.));

    let (app, startup_task) = App::with_windows(
        flags,
        ui.as_weak(),
        configure.as_weak(),
        scx_window.as_weak(),
    );
    let app = Arc::new(Mutex::new(app));

    wire_main_window(&ui, &app);
    wire_configure_window(&configure, &app);
    wire_scx_window(&scx_window, &app);

    // the initial presentation + the startup discovery
    app.lock().map(|a| a.sync_ui()).ok();
    dispatch(startup_task, &app);

    ui.run()
}

/// Route one UI event through the semantic update + the presentation sync.
fn dispatch_event(app: &Arc<Mutex<App>>, message: UiMessage) {
    let Ok(mut state) = app.lock() else {
        return;
    };
    let task = update(&mut state, message);
    state.sync_ui();
    drop(state);
    dispatch(task, app);
}

/// Wire the main-window callbacks to the courted semantic messages.
fn wire_main_window(ui: &MainWindow, app: &Arc<Mutex<App>>) {
    let app_execute = app.clone();
    ui.on_execute_clicked(move || {
        dispatch_event(&app_execute, UiMessage::Semantic(Message::ExecuteRequested))
    });
    let app_configure = app.clone();
    ui.on_configure_clicked(move || {
        dispatch_event(
            &app_configure,
            UiMessage::Semantic(Message::ConfigureRequested),
        )
    });
    let app_cancel = app.clone();
    ui.on_cancel_clicked(move || {
        dispatch_event(&app_cancel, UiMessage::Semantic(Message::CancelRequested))
    });
    let app_schedext = app.clone();
    ui.on_schedext_clicked(move || {
        dispatch_event(
            &app_schedext,
            UiMessage::Semantic(Message::SchedextRequested),
        )
    });
    let app_toggle = app.clone();
    ui.on_row_toggled(move |row| {
        // the presentation index -> the STABLE raw identity, then the
        // semantic message (sorting cannot redirect a click)
        let raw = app_toggle
            .lock()
            .ok()
            .and_then(|s| s.sorted_rows.get(row as usize).map(|r| r.raw.clone()));
        if let Some(raw) = raw {
            dispatch_event(
                &app_toggle,
                UiMessage::Semantic(Message::KernelToggled { raw }),
            );
        }
    });
    let app_sort = app.clone();
    ui.on_sort_clicked(move |column| {
        dispatch_event(
            &app_sort,
            UiMessage::SortRequested {
                column: column as usize,
            },
        )
    });
    // the shared dialog overlay callbacks (same semantics for every window)
    let app_err = app.clone();
    ui.on_dialog_error_dismissed(move || dispatch_event(&app_err, UiMessage::DialogDismissed));
    let app_conf = app.clone();
    ui.on_dialog_confirm_answered(move |yes| {
        dispatch_event(&app_conf, UiMessage::InstallQuestion(yes));
    });
    let app_path = app.clone();
    ui.on_dialog_path_dismissed(move || dispatch_event(&app_path, UiMessage::PathDialogDismissed));
    let app_path_sub = app.clone();
    ui.on_dialog_path_submitted(move |value| {
        dispatch_event(
            &app_path_sub,
            UiMessage::PathDialogChanged(value.to_string()),
        );
        dispatch_event(&app_path_sub, UiMessage::PathDialogSubmitted);
    });
}

/// Wire the Configure window: combos/checkboxes/patches/name/buttons map to
/// the semantic UI messages. Combo callbacks carry the SELECTED LABEL
/// string (Slint 1.17 has no index-change callback); the label is mapped
/// back through the CURRENT item list, so a stale value from a model change
/// is ignored.
fn wire_configure_window(ui: &ConfigureWindow, app: &Arc<Mutex<App>>) {
    let a = app.clone();
    ui.on_variant_selected(move |label| {
        let variant = a.lock().ok().and_then(|s| {
            KernelVariant::ALL
                .iter()
                .find(|v| {
                    s.tr(
                        tr_ctx::CONF_OPTIONS,
                        crate::configure_window::variant_label(**v),
                    ) == label.as_str()
                })
                .copied()
        });
        if let Some(variant) = variant {
            dispatch_event(&a, UiMessage::VariantPicked(variant));
        }
    });
    let a = app.clone();
    ui.on_check_toggled(move |index| {
        if let Some(check) = conf_check_at(index as usize) {
            dispatch_event(&a, UiMessage::CheckToggled(check));
        }
    });
    let a = app.clone();
    ui.on_lto_selected(move |label| {
        let value = a.lock().ok().and_then(|s| {
            s.configure
                .switch
                .lto_items
                .iter()
                .find(|m| lto_label(**m) == label.as_str())
                .copied()
        });
        if let Some(lto) = value {
            dispatch_event(&a, UiMessage::LtoPicked(lto));
        }
    });
    let a = app.clone();
    ui.on_preempt_selected(move |label| {
        let value = a.lock().ok().and_then(|s| {
            s.configure
                .switch
                .preempt_items
                .iter()
                .find(|m| preempt_label(**m) == label.as_str())
                .copied()
        });
        if let Some(preempt) = value {
            dispatch_event(&a, UiMessage::PreemptPicked(preempt));
        }
    });
    let a = app.clone();
    ui.on_hz_selected(move |label| {
        let hz = strings::combo_options::HZ_TICKS
            .iter()
            .position(|l| *l == label.as_str())
            .and_then(|i| HzTick::ALL.get(i))
            .copied();
        if let Some(hz) = hz {
            dispatch_event(&a, UiMessage::HzPicked(hz));
        }
    });
    let a = app.clone();
    ui.on_tickless_selected(move |label| {
        let mode = strings::combo_options::TICKLESS
            .iter()
            .position(|l| *l == label.as_str())
            .and_then(|i| TicklessMode::ALL.get(i))
            .copied();
        if let Some(mode) = mode {
            dispatch_event(&a, UiMessage::TicklessPicked(mode));
        }
    });
    let a = app.clone();
    ui.on_hugepage_selected(move |label| {
        let mode = strings::combo_options::HUGE_PAGE
            .iter()
            .position(|l| *l == label.as_str())
            .and_then(|i| HugepageMode::ALL.get(i))
            .copied();
        if let Some(mode) = mode {
            dispatch_event(&a, UiMessage::HugepagePicked(mode));
        }
    });
    let a = app.clone();
    ui.on_cpuopt_selected(move |label| {
        let mode = strings::combo_options::CPU_OPT
            .iter()
            .position(|l| *l == label.as_str())
            .and_then(|i| CpuOptMode::ALL.get(i))
            .copied();
        if let Some(mode) = mode {
            dispatch_event(&a, UiMessage::CpuOptPicked(mode));
        }
    });
    let a = app.clone();
    ui.on_custom_name_changed(move |value| {
        dispatch_event(&a, UiMessage::CustomNameChanged(value.to_string()));
    });
    let a = app.clone();
    ui.on_patches_add_local(move || {
        dispatch_event(
            &a,
            UiMessage::PathDialogOpened(PathDialogKind::AddLocalPatch),
        );
    });
    let a = app.clone();
    ui.on_patches_add_remote(move || {
        dispatch_event(
            &a,
            UiMessage::PathDialogOpened(PathDialogKind::AddRemotePatch),
        );
    });
    let a = app.clone();
    ui.on_patch_remove(move |index| {
        dispatch_event(&a, UiMessage::PatchOp(ConfPatchOp::Remove(index as usize)));
    });
    let a = app.clone();
    ui.on_patch_up(move |index| {
        dispatch_event(&a, UiMessage::PatchOp(ConfPatchOp::MoveUp(index as usize)));
    });
    let a = app.clone();
    ui.on_patch_down(move |index| {
        dispatch_event(
            &a,
            UiMessage::PatchOp(ConfPatchOp::MoveDown(index as usize)),
        );
    });
    let a = app.clone();
    ui.on_save_clicked(move || {
        dispatch_event(&a, UiMessage::PathDialogOpened(PathDialogKind::SaveConfig));
    });
    let a = app.clone();
    ui.on_load_clicked(move || {
        dispatch_event(&a, UiMessage::PathDialogOpened(PathDialogKind::LoadConfig));
    });
    let a = app.clone();
    ui.on_execute_clicked(move || {
        dispatch_event(&a, UiMessage::Semantic(Message::BuildRequested));
    });
    let a = app.clone();
    ui.on_cancel_clicked(move || {
        dispatch_event(
            &a,
            UiMessage::Semantic(Message::ConfigurationCancelRequested),
        );
    });
    // the shared dialog overlay callbacks
    let a = app.clone();
    ui.on_dialog_error_dismissed(move || dispatch_event(&a, UiMessage::DialogDismissed));
    let a = app.clone();
    ui.on_dialog_confirm_answered(move |yes| {
        dispatch_event(&a, UiMessage::InstallQuestion(yes));
    });
    let a = app.clone();
    ui.on_dialog_path_dismissed(move || dispatch_event(&a, UiMessage::PathDialogDismissed));
    let a = app.clone();
    ui.on_dialog_path_submitted(move |value| {
        dispatch_event(&a, UiMessage::PathDialogChanged(value.to_string()));
        dispatch_event(&a, UiMessage::PathDialogSubmitted);
    });
    // the WM close: closes ONLY the Configure window (ConfigurationCloseRequested)
    let a = app.clone();
    ui.window().on_close_requested(move || {
        dispatch_event(
            &a,
            UiMessage::Semantic(Message::ConfigurationCloseRequested),
        );
        slint::CloseRequestResponse::HideWindow
    });
}

/// Wire the sched-ext window (combos, flags, apply/disable/close).
fn wire_scx_window(ui: &SchedExtWindow, app: &Arc<Mutex<App>>) {
    let a = app.clone();
    ui.on_scheduler_selected(move |label| {
        let scheduler = a.lock().ok().and_then(|s| {
            s.scx.as_ref().and_then(|m| {
                m.schedulers
                    .iter()
                    .find(|s| s.as_str() == label.as_str())
                    .cloned()
            })
        });
        if let Some(scheduler) = scheduler {
            dispatch_event(&a, UiMessage::ScxSchedulerPicked(scheduler));
        }
    });
    let a = app.clone();
    ui.on_profile_selected(move |label| {
        if strings::combo_options::SCX_PROFILE.contains(&label.as_str()) {
            dispatch_event(&a, UiMessage::ScxProfilePicked(label.to_string()));
        }
    });
    let a = app.clone();
    ui.on_flags_changed(move |value| {
        dispatch_event(&a, UiMessage::ScxFlagsChanged(value.to_string()));
    });
    let a = app.clone();
    ui.on_apply_clicked(move || dispatch_event(&a, UiMessage::ScxApply));
    let a = app.clone();
    ui.on_disable_clicked(move || dispatch_event(&a, UiMessage::ScxDisable));
    let a = app.clone();
    ui.on_cancel_clicked(move || {
        dispatch_event(&a, UiMessage::Semantic(Message::ScxCloseRequested));
    });
    // the shared dialog overlay callbacks
    let a = app.clone();
    ui.on_dialog_error_dismissed(move || dispatch_event(&a, UiMessage::DialogDismissed));
    let a = app.clone();
    ui.on_dialog_confirm_answered(move |yes| {
        dispatch_event(&a, UiMessage::InstallQuestion(yes));
    });
    let a = app.clone();
    ui.on_dialog_path_dismissed(move || dispatch_event(&a, UiMessage::PathDialogDismissed));
    let a = app.clone();
    ui.on_dialog_path_submitted(move |value| {
        dispatch_event(&a, UiMessage::PathDialogChanged(value.to_string()));
        dispatch_event(&a, UiMessage::PathDialogSubmitted);
    });
    // the WM close: hides ONLY the sched-ext window (the app stays alive)
    let a = app.clone();
    ui.window().on_close_requested(move || {
        dispatch_event(&a, UiMessage::Semantic(Message::ScxCloseRequested));
        slint::CloseRequestResponse::HideWindow
    });
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
            verbose: false,
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
    fn transaction_returns_to_idle_so_execute_reenables() {
        // review seam #1: the transaction used to park in Complete/Failed
        // forever, soft-locking the OK button for the process lifetime.
        let mut app = app();
        // a dirty selection so execute_enabled() can come back on
        app.state
            .selection
            .rows
            .push(cachyos_kernel_manager_core::selection::KernelRow {
                raw: "cachyos/linux-cachyos".into(),
                name: "linux-cachyos".into(),
                installed: false,
                immutable: false,
                update_available: false,
                checked: true,
            });
        let _ = update(&mut app, UiMessage::Semantic(Message::ExecuteRequested));
        assert!(app.state.transaction_in_progress());
        // (a) NOTHING changed: back to Idle immediately, Execute re-enabled
        let _ = update(&mut app, UiMessage::TransactionFinished { changed: false });
        assert_eq!(app.state.transaction, TransactionState::Idle);
        assert!(app.state.execute_enabled());
        // (b) changed: Complete + refresh, then the refresh discovery -> Idle
        let _ = update(&mut app, UiMessage::Semantic(Message::ExecuteRequested));
        let _ = update(&mut app, UiMessage::TransactionFinished { changed: true });
        assert_eq!(
            app.state.transaction,
            TransactionState::Complete { changed: true }
        );
        assert!(!app.state.execute_enabled()); // refresh pending
        let _ = update(
            &mut app,
            UiMessage::CatalogLoaded(Box::new(CatalogPayload {
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
            })),
        );
        assert_eq!(app.state.transaction, TransactionState::Idle);
        // (c) failed: the error dialog ack releases the transaction
        let _ = update(&mut app, UiMessage::Semantic(Message::ExecuteRequested));
        let _ = update(
            &mut app,
            UiMessage::TransactionFailed {
                message: "alpm init failed".into(),
            },
        );
        assert_eq!(app.state.transaction, TransactionState::Failed);
        assert!(!app.state.execute_enabled());
        let _ = update(&mut app, UiMessage::DialogDismissed);
        assert_eq!(app.state.transaction, TransactionState::Idle);
        assert!(app.state.execute_enabled());
    }

    #[test]
    fn catalog_load_populates_rows_and_readies_the_app() {
        // a NON-empty catalog readies the app with NO dialog (the
        // "No kernels found" dialog is reserved for a genuinely empty
        // catalog — audit P1)
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![KernelRowView {
                raw: "cachyos/linux-cachyos".into(),
                version_text: "7.1.8-1".into(),
                category: "cachyos".into(),
                checked: true,
                immutable: true,
                update_available: false,
            }],
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
        // the oracle's "No kernels found" path (`init_kernels`, km-window.cpp:
        // 228-230): an empty catalog readies the app (the tree renders empty;
        // the OK stays disabled) AND raises the critical dialog. The old code
        // treated it as a normal Ready with NO dialog — indistinguishable from
        // the fail-open ALPM-panic masquerade (audit P1).
        let mut app = app();
        let payload = CatalogPayload {
            rows: vec![],
            kernels: BTreeMap::new(),
            installed: BTreeMap::new(),
            hardware: HardwareProfile::default(),
        };
        let _ = update(&mut app, UiMessage::CatalogLoaded(Box::new(payload)));
        assert!(!app.state.execute_enabled());
        assert!(matches!(app.state.dialogs, DialogsState::Error { .. }));
    }

    #[test]
    fn discovery_failure_is_an_error_dialog_not_an_empty_catalog() {
        // audit P1 fail-closed: an ALPM init failure must NEVER look like
        // "discovery succeeded, zero kernels" — the old `.expect()` panicked
        // and `blocking`'s `A::default()` fallback produced exactly that
        // valid-looking empty catalog.
        let mut app = app();
        let _ = update(
            &mut app,
            UiMessage::DiscoveryFailed("alpm init failed: x".into()),
        );
        assert!(matches!(app.state.dialogs, DialogsState::Error { .. }));
        assert!(app.rows.is_empty());
        // the app readies (the progress dialog closes, the OK stays disabled)
        assert_eq!(app.state.lifecycle, LifecycleState::Ready);
        assert!(!app.state.execute_enabled());
    }

    #[test]
    fn empty_patch_submissions_are_ignored() {
        // audit P2: the oracle's local picker returns on an empty selection
        // and the remote input returns on an empty URL (conf-window.cpp:
        // 620-622, 643-646); the candidate must not turn an empty
        // submission into a `file://` or bare-empty patch entry.
        let mut app = app();
        let _ = update(
            &mut app,
            UiMessage::PathDialogOpened(PathDialogKind::AddLocalPatch),
        );
        let _ = update(&mut app, UiMessage::PathDialogChanged(String::new()));
        let _ = update(&mut app, UiMessage::PathDialogSubmitted);
        assert!(app.configure.patches.is_empty());
        let _ = update(
            &mut app,
            UiMessage::PathDialogOpened(PathDialogKind::AddRemotePatch),
        );
        let _ = update(&mut app, UiMessage::PathDialogChanged("   ".into()));
        let _ = update(&mut app, UiMessage::PathDialogSubmitted);
        assert!(app.configure.patches.is_empty());
        // a real value still lands (with the file:// prefix for local)
        let _ = update(
            &mut app,
            UiMessage::PathDialogOpened(PathDialogKind::AddLocalPatch),
        );
        let _ = update(
            &mut app,
            UiMessage::PathDialogChanged("/tmp/real.patch".into()),
        );
        let _ = update(&mut app, UiMessage::PathDialogSubmitted);
        assert_eq!(
            app.configure.patches,
            vec!["file:///tmp/real.patch".to_string()]
        );
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
            UiMessage::Semantic(Message::KernelToggled {
                raw: "cachyos/linux-cachyos".into(),
            }),
        );
        assert!(app.state.execute_enabled());
        assert_eq!(app.state.phase(), AppPhase::SelectionChanged);
    }

    #[test]
    fn configure_cancel_while_build_running_bumps_the_operation_generation() {
        // the review's process-ownership seam (D-008): closing Configure
        // while a build runs must terminate the in-flight operation, not
        // just close the window. The production cancel hook bumps the
        // operation GENERATION (invalidating the in-flight worker — the
        // pre/post-spawn epoch checks abort it) and would kill the owned
        // terminal-helper child; the worker then reports the FAILURE branch.
        let mut app = app();
        let epoch_before = app.build_epoch.load(Ordering::Relaxed);
        app.state.build = BuildState::Running;
        let _ = update(
            &mut app,
            UiMessage::Semantic(Message::ConfigurationCancelRequested),
        );
        assert_eq!(
            app.build_epoch.load(Ordering::Relaxed),
            epoch_before + 1,
            "the operation generation must be bumped on cancel"
        );
        // the window closes, the app stays alive (NOT shutting down)
        assert_eq!(app.state.configuration, ConfigurationState::Closed);
        assert_ne!(app.state.lifecycle, LifecycleState::Shutdown);
        // a cancel with NO build in flight does not bump the generation
        let mut app2 = App::new(flags()).0;
        let epoch2 = app2.build_epoch.load(Ordering::Relaxed);
        assert_eq!(app2.state.build, BuildState::Idle);
        let _ = update(
            &mut app2,
            UiMessage::Semantic(Message::ConfigurationCancelRequested),
        );
        assert_eq!(app2.build_epoch.load(Ordering::Relaxed), epoch2);
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
        let _ = update(
            &mut app,
            UiMessage::ConfigurePrepared {
                generation: 1,
                patches: vec![],
            },
        );
        assert_eq!(app.state.configuration, ConfigurationState::Editing);
    }

    #[test]
    fn variant_switch_updates_the_configure_model() {
        let mut app = app();
        app.configure = ConfigureWindowModel::default();
        let _ = update(&mut app, UiMessage::VariantPicked(KernelVariant::Hardened));
        assert_eq!(app.configure.variant, KernelVariant::Hardened);
        assert_eq!(app.configure.switch.lto_selected, LtoMode::None);
        // the core build_options follow (the RunBuild effect's dir)
        assert_eq!(app.state.build_options.variant, KernelVariant::Hardened);
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
    fn stale_patch_refresh_is_discarded_by_generation() {
        // audit P1: a rapid A→B variant change can finish B first and A
        // second; the OLD handler blindly replaced the list with whichever
        // worker finished last. The generation epoch must discard the stale
        // completion and keep the newest patches.
        let mut app = app();
        app.patch_epoch = 2; // a variant switch bumped the epoch twice
                             // the stale (older-generation) probe result lands LAST: discarded
        let _ = update(
            &mut app,
            UiMessage::PatchesRefreshed {
                generation: 1,
                patches: vec!["stale.patch".into()],
            },
        );
        assert!(app.configure.patches.is_empty());
        // the current-generation result lands: applied
        let _ = update(
            &mut app,
            UiMessage::PatchesRefreshed {
                generation: 2,
                patches: vec!["current.patch".into()],
            },
        );
        assert_eq!(app.configure.patches, vec!["current.patch".to_string()]);
        // a patch mutation bumps the epoch: an older in-flight refresh can
        // never erase the user's edit
        let _ = update(&mut app, UiMessage::PatchOp(ConfPatchOp::MoveUp(0)));
        let _ = update(
            &mut app,
            UiMessage::PatchesRefreshed {
                generation: 2,
                patches: vec!["older.patch".into()],
            },
        );
        assert_eq!(app.configure.patches, vec!["current.patch".to_string()]);
    }

    #[test]
    fn build_success_asks_the_install_question() {
        let mut app = app();
        let _ = update(&mut app, UiMessage::Semantic(Message::ConfigureRequested));
        let _ = update(
            &mut app,
            UiMessage::ConfigurePrepared {
                generation: 1,
                patches: vec![],
            },
        );
        let _ = update(&mut app, UiMessage::BuildFinished { success: true });
        assert_eq!(app.state.build, BuildState::AwaitingInstallDecision);
        assert!(matches!(app.state.dialogs, DialogsState::Confirm { .. }));
        // Yes -> the artifact install phase
        let _ = update(&mut app, UiMessage::InstallQuestion(true));
        assert_eq!(app.state.build, BuildState::Installing);
        let _ = update(&mut app, UiMessage::ArtifactsInstalled);
        assert_eq!(app.state.build, BuildState::Idle);
        // No -> immediately retryable (the audit P0 soft-lock regression:
        // the old Completed state stayed until the app exited, silently
        // ignoring every later Build)
        let _ = update(&mut app, UiMessage::BuildFinished { success: true });
        let _ = update(&mut app, UiMessage::InstallQuestion(false));
        assert_eq!(app.state.build, BuildState::Idle);
        // and a FAILED build is immediately retryable too
        let _ = update(&mut app, UiMessage::BuildFinished { success: false });
        assert_eq!(app.state.build, BuildState::Failed);
        let _ = update(&mut app, UiMessage::Semantic(Message::BuildRequested));
        assert_eq!(app.state.build, BuildState::Running);
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
                                                 // clicking a DIFFERENT column resets to ASCENDING (the oracle's
                                                 // QTreeWidget `_q_headerClicked` — witnessed by ui/gui-drive 2026-08-
                                                 // 23: after the Choose click flips column 0 to descending, the
                                                 // PkgName/Version/Category clicks all produce ASCENDING orders with
                                                 // the previous order as the stable tie-break). Column 0's keys are
                                                 // all equal, so the ASCENDING col-0 click keeps the displayed base
                                                 // order ([b, a]) exactly.
        let _ = update(&mut app, UiMessage::SortRequested { column: 0 });
        assert_eq!(app.sorted_rows[0].raw, "b"); // col-0 asc: all-equal, stable base
                                                 // the SAME column toggles: col-0 desc is still all-equal -> unchanged
        let _ = update(&mut app, UiMessage::SortRequested { column: 0 });
        assert_eq!(app.sorted_rows[0].raw, "b");
        let _ = update(&mut app, UiMessage::SortRequested { column: 1 });
        assert_eq!(app.sorted_rows[0].raw, "a"); // ASCENDING again, not descending
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

    #[test]
    fn remaining_option_combos_feed_the_model() {
        // review seam #9: tickless/hugepage/cpu-opt were no-ops — they now
        // write the configure model AND the core build_options
        let mut app = app();
        let _ = update(&mut app, UiMessage::TicklessPicked(TicklessMode::Idle));
        let _ = update(&mut app, UiMessage::HugepagePicked(HugepageMode::Madvise));
        let _ = update(&mut app, UiMessage::CpuOptPicked(CpuOptMode::Native));
        assert_eq!(app.configure.tickless, TicklessMode::Idle);
        assert_eq!(app.configure.hugepage, HugepageMode::Madvise);
        assert_eq!(app.configure.cpu_opt, CpuOptMode::Native);
        assert_eq!(app.state.build_options.tickless, TicklessMode::Idle);
        assert_eq!(app.state.build_options.hugepage, HugepageMode::Madvise);
        assert_eq!(app.state.build_options.cpu_opt, CpuOptMode::Native);
        // a checkbox change mirrors into build_options too (seam #6: the
        // build env string must reflect the real UI state)
        let _ = update(&mut app, UiMessage::CheckToggled(ConfCheck::PerGov));
        assert_eq!(app.configure.per_gov_checked, true);
        assert_eq!(app.state.build_options.per_gov, true);
        let _ = update(&mut app, UiMessage::CustomNameChanged("my-kernel".into()));
        assert_eq!(app.state.build_options.custom_name, "my-kernel");
    }

    #[test]
    fn scx_window_close_hides_only_the_scx_window() {
        let mut app = app();
        // reach Ready (the schedext button requires a loaded catalog)
        let _ = update(
            &mut app,
            UiMessage::CatalogLoaded(Box::new(CatalogPayload {
                rows: vec![],
                kernels: BTreeMap::new(),
                installed: BTreeMap::new(),
                hardware: HardwareProfile::default(),
            })),
        );
        let _ = update(&mut app, UiMessage::Semantic(Message::SchedextRequested));
        assert_eq!(app.state.scx, ScxState::Visible);
        let _ = update(&mut app, UiMessage::ScxWindowClosed);
        assert_eq!(app.state.scx, ScxState::Hidden);
        // the app is still alive (no Close effect)
        assert_eq!(app.state.lifecycle, LifecycleState::Ready);
    }

    use cachyos_kernel_manager_core::options::LtoMode;
    use cachyos_kernel_manager_core::options::{CpuOptMode, HugepageMode, TicklessMode};
    use cachyos_kernel_manager_core::state::ScxState;
}
