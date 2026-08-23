//! Pure domain model for CachyOS Kernel Manager.
//!
//! This crate is the semantic core. It must never import any presentation
//! technology (Iced, Slint, Qt) and must never execute external programs.
//! Everything here is a reconstruction of the frozen oracle's behavior
//! (oracle revision `6b4a373e`, v1.19.0) as pure data + pure functions,
//! so the logic can be unit-tested, property-tested, and differentially
//! courted without a GUI or a real package database.
//!
//! Provenance discipline: each module names the upstream source file(s) it
//! reconstructs and the court that protects it (see `docs/COURTS.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod discovery;
pub mod kernel;
pub mod options;
pub mod selection;
pub mod state;

pub use discovery::{companions_for, discover_kernels, DbPackage, DiscoveredKernel, SyncDb};
pub use kernel::{
    classify_category, kernel_headers_name, matches_headers_needle, DisplayVersion, KernelCategory,
    KernelName, UpdateFlag, AUR_VERSION, DOWNGRADE_MARKER, UPDATE_MARKER,
};
pub use options::{
    BuildOptions, CpuOptMode, HugepageMode, HzTick, KernelVariant, LtoMode, PreemptMode,
    TicklessMode, VariantTransitions, CHECKBOX_BINDINGS,
};
pub use selection::{toggle_row, KernelRow, SelectionState, IMMUTABLE};
pub use state::{AppEvent, AppPhase, AppState, Effect};
