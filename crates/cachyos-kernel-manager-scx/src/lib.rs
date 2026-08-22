//! sched-ext / scx_loader integration (Phase 7).
//!
//! The frozen oracle embeds the external `scxctl-ui` library
//! (`km-window.hpp:47,144`); that library was extracted FROM this
//! repository, and its final in-repo state (commit `f3eeaf6`) plus the
//! pinned `scx_loader 1.0.9` crate are the recoverable SCX authority
//! (`oracle/scx-authority/SCX-AUTHORITY.md`, `oracle/UPSTREAM.lock [scx]`).
//!
//! Architecture:
//! ```text
//! Iced UI → typed Rust SCX client → D-Bus (org.scx.Loader) → scx_loader
//! ```
//!
//! The crate is layered so the courts can pin every decision without the
//! D-Bus transport:
//! - [`config`] — `SupportedSched`/`SchedMode`, the default per-mode flag
//!   matrix, the `scx_loader.toml` shape;
//! - [`interface`] — the typed `org.scx.Loader` surface as a pure,
//!   inspectable declaration (the single source the zbus proxy and the
//!   `scx-introspect` witness are generated from);
//! - [`state`] — the sysfs current-scheduler readback;
//! - [`apply`] — the apply/disable decision traces (service disable,
//!   args-vs-mode, loader enable, pkexec copy);
//! - [`window`] — the main-window button visibility + SchedExtWindow
//!   init/profile/apply/disable UI decisions;
//! - [`client`] (`dbus` feature) — the zbus client implementing the
//!   declared interface.
//!
//! Courts: `scx/*` (button visibility, current scheduler, mode flags,
//! window init, apply, disable, loader interface).

#![forbid(unsafe_code)]

pub mod apply;
pub mod config;
pub mod interface;
pub mod state;
pub mod window;

#[cfg(feature = "dbus")]
pub mod client;

pub use config::{SchedConfig, SchedFlags, SchedMode, SupportedSched};
pub use interface::{loader_interface, InterfaceDesc, MethodDesc, PropertyDesc};
