//! sched-ext / scx_loader integration.
//!
//! Phase 7. What is *known* from the frozen oracle (revision `6b4a373e`):
//!
//! - The main window embeds `scxctl::SchedExtWindow` from the external
//!   `scxctl-ui` library (`km-window.hpp:47,144`, CMake
//!   `find_package(scxctl-ui 1 REQUIRED)`).
//! - The sched-ext button is hidden unless `/sys/kernel/sched_ext/state`
//!   exists (`km-window.cpp:185-188`).
//! - History: the scx-manager UI was extracted from this repository; its
//!   D-Bus apply/disable logic was moved into Rust (commits `425681d`,
//!   `c866d99`). The D-Bus surface is `org.scx.Loader` (scx_loader).
//!
//! The preferred architecture (directive §29):
//! ```text
//! Iced UI → typed Rust SCX client → D-Bus → scx_loader
//! ```
//! Parity must be proven in VMs (`courts/scx/*`): button visibility, state
//! readback, apply/disable, loader restart, BMQ restrictions.
//!
//! This crate currently records the interface facts and the configuration
//! model; the D-Bus client is implemented in Phase 7.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The D-Bus service name of scx_loader.
pub const SCX_LOADER_DBUS_NAME: &str = "org.scx.Loader";

/// sysfs state file governing main-window button visibility
/// (`km-window.cpp:186`).
pub const SCHED_EXT_STATE_FILE: &str = "/sys/kernel/sched_ext/state";

/// Scheduler configuration as modeled by the candidate (Phase 7 refines
/// this against the real `org.scx.Loader` interface and scxctl-ui behavior).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerConfiguration {
    /// Selected scheduler (e.g. `scx_bpfland`).
    pub scheduler: String,
    /// Selected mode (e.g. `auto`).
    pub mode: String,
    /// Extra arguments, only passed when they differ from the scheduler
    /// defaults (commit `b70b01b`).
    pub args: Vec<String>,
}
