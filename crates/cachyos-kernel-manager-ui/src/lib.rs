//! Iced application — Phase 8.
//!
//! This crate currently defines the *semantic* message/state surface
//! (directive §7/§8) against the core domain types, with NO Iced dependency.
//! The rendering layer (Phase 8) will translate these messages into Iced
//! `Message`s and vice versa; the core domain remains presentation-free.
//!
//! Nothing in this crate is rendered yet; the phase is not claimed complete
//! (docs/ARCHITECTURE.md phase table).

#![forbid(unsafe_code)]

use cachyos_kernel_manager_core::KernelCategory;
use serde::{Deserialize, Serialize};

/// Semantic UI messages (directive §7 style). The UI never contains domain
/// logic like "if NVIDIA and this package exists, append that package" —
/// that lives in the plan crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    KernelToggled { row: usize },
    ExecuteRequested,
    ConfigureRequested,
    ConfigLoaded { config: KernelManagerConfig },
    PatchAdded { entry: String },
    PatchRemoved { index: usize },
    PatchMoved { from: usize, to: usize },
    SchedulerChanged { scheduler: String, mode: String },
    CancelRequested,
    CloseRequested,
    BuildRequested,
    InstallArtifactsRequested,
    VariantChanged { variant: KernelVariant },
}

pub use cachyos_kernel_manager_core::options::KernelVariant;

/// UI-side view model of one kernel row (rendering concerns only: text,
/// state; the semantics stay in core).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelRowView {
    pub raw: String,
    pub version_text: String,
    pub category: KernelCategory,
    pub checked: bool,
    pub immutable: bool,
    pub update_available: bool,
}

/// Placeholder re-export to keep the config surface obvious in the UI crate;
/// the config crate is the owner.
pub use cachyos_kernel_manager_config::KernelManagerConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_serializable_and_semantic() {
        let m = Message::KernelToggled { row: 3 };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, r#"{"KernelToggled":{"row":3}}"#);
    }
}
