//! `cachyos-kernel-manager-gui` — the Phase 8 Iced application entry
//! (feature `rendering`).
//!
//! The shipped `cachyos-kernel-manager` binary launches the same app
//! (`cachyos_kernel_manager_ui::app::run`); this bin is the standalone
//! development entry (`cargo run -p cachyos-kernel-manager-ui --bin
//! cachyos-kernel-manager-gui --features rendering`).

use cachyos_kernel_manager_ui::app::{Flags, UiMessage};

pub fn main() -> iced::Result {
    cachyos_kernel_manager_ui::app::run(Flags::from_env())
}

/// Keep the message type alive for the app's type-check (the iced runtime
/// requires `Message: Send + Debug + 'static`).
#[allow(dead_code)]
fn _typecheck(_: UiMessage) {}
