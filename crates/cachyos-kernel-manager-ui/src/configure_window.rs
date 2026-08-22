//! The Configure window's semantic model — the
//! `ui/configure-window-semantics` court's candidate side.
//!
//! Reconstructed from `conf-window.cpp` (revision `6b4a373e`):
//! - the ctor defaults (`conf-window.cpp:475-546`): the variant combo
//!   labels, the option checkboxes (hardly + cachy_config checked), the
//!   combo lists, the initial LTO selection (thin);
//! - the variant-switch handler (`conf-window.cpp:553-602`, delegated to
//!   core's `VariantSwitchState` — courted by option-transitions);
//! - the patches tab (`conf-window.cpp:453-473,607-686`): the source-array
//!   probe filtered to `.patch`, the local-patch `file://` prefix, the
//!   remote URL input, remove/move-up/move-down;
//! - the save/load flows (`conf-window.cpp:737-810`);
//! - the Build-kernel flow (`on_execute`, `conf-window.cpp:696-735` —
//!   courted by build-env/lifecycle + the build crate).

use crate::strings;
use cachyos_kernel_manager_core::options::{
    KernelVariant, VariantSwitchState, DEFAULT_CUSTOM_NAME, PKGBASE_SENTINEL,
};

/// The Configure window's model.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConfigureWindowModel {
    /// The selected variant (the main combo's index).
    pub variant: KernelVariant,
    /// The variant combo label (`conf-window.cpp:487-496`).
    pub variant_label: &'static str,
    /// The option-switch state (lto/preempt/hz items+selection, cachy_config,
    /// builtin_zfs).
    pub switch: VariantSwitchState,
    /// The `hardly` checkbox (checked at ctor, `conf-window.cpp:501`).
    pub hardly_checked: bool,
    /// The patches list (`reset_patches_data_tab` + the list operations).
    pub patches: Vec<String>,
    /// The custom-name field (`conf-options-page.ui:53` default).
    pub custom_name: String,
}

impl Default for ConfigureWindowModel {
    /// The initial window state (`conf-window.cpp:475-546`).
    fn default() -> Self {
        ConfigureWindowModel {
            variant: KernelVariant::Cachyos,
            variant_label: variant_label(KernelVariant::Cachyos),
            switch: VariantSwitchState::default(),
            hardly_checked: true,
            patches: Vec::new(),
            custom_name: DEFAULT_CUSTOM_NAME.to_string(),
        }
    }
}

/// The variant combo labels (`conf-window.cpp:487-496`), by variant.
pub fn variant_label(variant: KernelVariant) -> &'static str {
    match variant {
        KernelVariant::Cachyos => strings::VARIANT_LABELS[0],
        KernelVariant::Bore => strings::VARIANT_LABELS[1],
        KernelVariant::Rc => strings::VARIANT_LABELS[2],
        KernelVariant::Rt => strings::VARIANT_LABELS[3],
        KernelVariant::Lts => strings::VARIANT_LABELS[4],
        KernelVariant::Eevdf => strings::VARIANT_LABELS[5],
        KernelVariant::Bmq => strings::VARIANT_LABELS[6],
        KernelVariant::Hardened => strings::VARIANT_LABELS[7],
        KernelVariant::Deckify => strings::VARIANT_LABELS[8],
        KernelVariant::Server => strings::VARIANT_LABELS[9],
    }
}

impl ConfigureWindowModel {
    /// The `main_combo_box` change handler (`conf-window.cpp:553-602`) +
    /// `reset_patches_data_tab` (the probe result feeds the list).
    pub fn on_variant_changed(&mut self, variant: KernelVariant, source_array: &[String]) {
        self.variant = variant;
        self.variant_label = variant_label(variant);
        self.switch.switch_to(variant);
        self.reset_patches(source_array);
    }

    /// `reset_patches_data_tab` (`conf-window.cpp:458-473`): the source-array
    /// probe result, filtered to entries ending with `.patch`, replaces the
    /// list.
    pub fn reset_patches(&mut self, source_array: &[String]) {
        self.patches = source_array
            .iter()
            .filter(|item| item.ends_with(".patch"))
            .cloned()
            .collect();
    }

    /// The local-patch picker (`conf-window.cpp:615-634`): each selected file
    /// is prepended with `file://` and appended.
    pub fn add_local_patches(&mut self, files: &[String]) {
        for file in files {
            self.patches.push(format!("file://{file}"));
        }
    }

    /// The remote-patch URL input (`conf-window.cpp:636-651`): appended
    /// verbatim.
    pub fn add_remote_patch(&mut self, url: String) {
        self.patches.push(url);
    }

    /// The remove button (`conf-window.cpp:658-664`): the current row.
    pub fn remove_patch(&mut self, index: usize) {
        if index < self.patches.len() {
            self.patches.remove(index);
        }
    }

    /// The move-up button (`conf-window.cpp:667-675`): index > 0.
    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.patches.len() {
            self.patches.swap(index, index - 1);
        }
    }

    /// The move-down button (`conf-window.cpp:677-685`): not the last row.
    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.patches.len() {
            self.patches.swap(index, index + 1);
        }
    }

    /// `on_save`'s config mutation (`conf-window.cpp:737-756`): the option
    /// values the save writes. The full config serialization is the config
    /// crate's (`config-roundtrip` court); the UI model fixes which UI state
    /// feeds it.
    pub fn save_ui_state(&self) -> SaveUiState {
        SaveUiState {
            variant: self.variant,
            hardly: self.hardly_checked,
            cachy_config: self.switch.cachy_config_checked,
            lto: self.switch.lto_selected,
            preempt: self.switch.preempt_selected,
            hz_ticks: self.switch.hz_selected,
            custom_name: self.custom_name.clone(),
        }
    }

    /// `conf-window.cpp:446`: the `_use_lto_suffix=n` workaround fires when
    /// lto != none AND the custom name is not the `$pkgbase` sentinel.
    pub fn use_lto_suffix(&self) -> bool {
        self.switch.lto_selected != cachyos_kernel_manager_core::options::LtoMode::None
            && self.custom_name != PKGBASE_SENTINEL
    }
}

/// The UI state `on_save` feeds into the config (`conf-window.cpp:743-755`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaveUiState {
    pub variant: KernelVariant,
    pub hardly: bool,
    pub cachy_config: bool,
    pub lto: cachyos_kernel_manager_core::options::LtoMode,
    pub preempt: cachyos_kernel_manager_core::options::PreemptMode,
    pub hz_ticks: cachyos_kernel_manager_core::options::HzTick,
    pub custom_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachyos_kernel_manager_core::options::{HzTick, KernelVariant, LtoMode, PreemptMode};

    #[test]
    fn defaults_match_the_ctor() {
        let m = ConfigureWindowModel::default();
        assert_eq!(m.variant, KernelVariant::Cachyos);
        assert_eq!(m.variant_label, "CachyOS default Scheduler (tuned EEVDF)");
        assert!(m.hardly_checked);
        assert!(m.switch.cachy_config_checked);
        assert_eq!(m.switch.lto_selected, LtoMode::Thin);
        assert_eq!(
            m.switch.lto_items,
            vec![
                LtoMode::None,
                LtoMode::Full,
                LtoMode::Thin,
                LtoMode::ThinDist
            ]
        );
        assert_eq!(m.switch.preempt_selected, PreemptMode::Full);
        assert_eq!(m.custom_name, DEFAULT_CUSTOM_NAME);
    }

    #[test]
    fn variant_switch_to_hardened_removes_thin_dist_and_extends_preempt() {
        let mut m = ConfigureWindowModel::default();
        m.on_variant_changed(KernelVariant::Hardened, &[]);
        assert_eq!(
            m.switch.lto_items,
            vec![LtoMode::None, LtoMode::Full, LtoMode::Thin]
        );
        assert_eq!(m.switch.lto_selected, LtoMode::None);
        assert_eq!(
            m.switch.preempt_items,
            vec![
                PreemptMode::Full,
                PreemptMode::Lazy,
                PreemptMode::Voluntary,
                PreemptMode::None
            ]
        );
        assert_eq!(m.switch.preempt_selected, PreemptMode::Full);
    }

    #[test]
    fn variant_switch_to_server_sets_defaults() {
        let mut m = ConfigureWindowModel::default();
        m.on_variant_changed(KernelVariant::Server, &[]);
        assert_eq!(m.switch.preempt_selected, PreemptMode::Lazy);
        assert_eq!(m.switch.hz_selected, HzTick::Hz300);
        assert!(!m.switch.cachy_config_checked);
    }

    #[test]
    fn variant_switch_to_rt_disables_zfs() {
        let mut m = ConfigureWindowModel::default();
        m.switch.zfs_checked = true;
        m.on_variant_changed(KernelVariant::Rt, &[]);
        assert!(!m.switch.zfs_enabled);
        assert!(!m.switch.zfs_checked);
    }

    #[test]
    fn reset_patches_filters_to_dot_patch() {
        let mut m = ConfigureWindowModel::default();
        let source = vec![
            "https://example.invalid/kernel.tar.gz".to_string(),
            "patches/foo.patch".to_string(),
            "https://example.invalid/bar.patch".to_string(),
            "not-a-patch".to_string(),
        ];
        m.reset_patches(&source);
        assert_eq!(
            m.patches,
            vec!["patches/foo.patch", "https://example.invalid/bar.patch"]
        );
    }

    #[test]
    fn patch_ops_match_the_list_widget() {
        let mut m = ConfigureWindowModel::default();
        m.add_local_patches(&["/tmp/a.patch".into(), "/tmp/b.patch".into()]);
        assert_eq!(
            m.patches,
            vec!["file:///tmp/a.patch", "file:///tmp/b.patch"]
        );
        m.add_remote_patch("https://example.invalid/x.patch".into());
        assert_eq!(m.patches.len(), 3);
        m.move_up(2);
        assert_eq!(m.patches[1], "https://example.invalid/x.patch");
        m.move_down(0);
        assert_eq!(m.patches[0], "https://example.invalid/x.patch");
        m.remove_patch(1);
        assert_eq!(
            m.patches,
            vec!["https://example.invalid/x.patch", "file:///tmp/b.patch"]
        );
    }

    #[test]
    fn lto_suffix_workaround_condition() {
        let m = ConfigureWindowModel::default();
        // thin + $pkgbase-custom -> suffix
        assert!(m.use_lto_suffix());
        let mut sentinel = m.clone();
        sentinel.custom_name = PKGBASE_SENTINEL.to_string();
        assert!(!sentinel.use_lto_suffix());
        let mut none = m.clone();
        none.switch.lto_selected = LtoMode::None;
        assert!(!none.use_lto_suffix());
    }
}
