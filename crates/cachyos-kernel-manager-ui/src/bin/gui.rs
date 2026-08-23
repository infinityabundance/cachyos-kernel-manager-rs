//! `cachyos-kernel-manager-gui` — the Phase 8 Slint application entry
//! (feature `rendering`).
//!
//! The shipped `cachyos-kernel-manager` binary launches the same app
//! (`cachyos_kernel_manager_ui::app::run`); this bin is the standalone
//! development entry (`cargo run -p cachyos-kernel-manager-ui --bin
//! cachyos-kernel-manager-gui --features rendering`).

use cachyos_kernel_manager_ui::app::Flags;

pub fn main() -> Result<(), slint::PlatformError> {
    cachyos_kernel_manager_ui::app::run(Flags::from_env())
}
