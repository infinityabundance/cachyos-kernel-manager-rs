//! The typed D-Bus client over `org.scx.Loader` (zbus 5.5.0 / zvariant
//! 5.4.0 — the EXACT versions the frozen authority's
//! `config-option-lib/Cargo.lock` pins, so the wire encoding matches
//! `scx_loader 1.0.9` byte-for-byte).
//!
//! Mirrors `scx_loader/src/dbus.rs` (`LoaderClientProxy`) exactly:
//! interface/service `org.scx.Loader`, path `/org/scx/Loader`, the five
//! methods and three read properties. `SupportedSched` carries
//! `#[zvariant(signature = "s")]`; `SchedMode` is a repr-less fieldless
//! enum → u32 (the derives live on the types in [`crate::config`]).

use crate::config::{SchedMode, SupportedSched};
use zbus::proxy;

#[proxy(
    interface = "org.scx.Loader",
    default_service = "org.scx.Loader",
    default_path = "/org/scx/Loader"
)]
pub trait LoaderClient {
    /// Starts the specified scheduler with the given mode.
    fn start_scheduler(&self, scx_name: &SupportedSched, sched_mode: SchedMode)
        -> zbus::Result<()>;

    /// Starts the specified scheduler with the provided arguments.
    fn start_scheduler_with_args(
        &self,
        scx_name: &SupportedSched,
        scx_args: &[String],
    ) -> zbus::Result<()>;

    /// Stops the currently running scheduler.
    fn stop_scheduler(&self) -> zbus::Result<()>;

    /// Switches to the specified scheduler with the given mode (stops the
    /// current scheduler first, if any).
    fn switch_scheduler(
        &self,
        scx_name: &SupportedSched,
        sched_mode: SchedMode,
    ) -> zbus::Result<()>;

    /// Switches to the specified scheduler with the provided arguments.
    fn switch_scheduler_with_args(
        &self,
        scx_name: &SupportedSched,
        scx_args: &[String],
    ) -> zbus::Result<()>;

    /// The name of the currently running scheduler ("unknown" if none).
    #[zbus(property)]
    fn current_scheduler(&self) -> zbus::Result<String>;

    /// The currently active scheduler mode (0 = Auto if none).
    #[zbus(property)]
    fn scheduler_mode(&self) -> zbus::Result<SchedMode>;

    /// The schedulers currently supported by the loader.
    #[zbus(property)]
    fn supported_schedulers(&self) -> zbus::Result<Vec<String>>;
}
