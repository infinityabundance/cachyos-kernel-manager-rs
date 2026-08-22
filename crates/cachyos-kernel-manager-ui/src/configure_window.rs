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
use cachyos_kernel_manager_config::KernelManagerConfig;
use cachyos_kernel_manager_core::options::{
    CpuOptMode, HugepageMode, HzTick, KernelVariant, TicklessMode, VariantSwitchState,
    DEFAULT_CUSTOM_NAME, PKGBASE_SENTINEL,
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
    /// The remaining option checkboxes (unchecked at ctor — Qt's default;
    /// `conf-window.cpp:497-499` only checks cachy_config + hardly).
    pub per_gov_checked: bool,
    pub tcp_bbr3_checked: bool,
    pub nconfig_checked: bool,
    pub xconfig_checked: bool,
    pub localmodcfg_checked: bool,
    pub use_current_checked: bool,
    pub builtin_nvidia_open_checked: bool,
    pub build_debug_checked: bool,
    /// The tickless combo (default index 0 = Full).
    pub tickless: TicklessMode,
    /// The hugepage combo (default index 0 = Always).
    pub hugepage: HugepageMode,
    /// The cpu-opt combo (default index 0 = Disabled).
    pub cpu_opt: CpuOptMode,
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
            per_gov_checked: false,
            tcp_bbr3_checked: false,
            nconfig_checked: false,
            xconfig_checked: false,
            localmodcfg_checked: false,
            use_current_checked: false,
            builtin_nvidia_open_checked: false,
            build_debug_checked: false,
            tickless: TicklessMode::Full,
            hugepage: HugepageMode::Always,
            cpu_opt: CpuOptMode::Manual,
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

    /// The full config `on_save` writes (`conf-window.cpp:737-756`): the
    /// 18-field `KernelManagerConfig` from the current widget state.
    pub fn to_config(&self) -> KernelManagerConfig {
        KernelManagerConfig {
            hardly_check: self.hardly_checked,
            per_gov_check: self.per_gov_checked,
            tcp_bbr3_check: self.tcp_bbr3_checked,
            cachy_config_check: self.switch.cachy_config_checked,
            nconfig_check: self.nconfig_checked,
            xconfig_check: self.xconfig_checked,
            localmodcfg_check: self.localmodcfg_checked,
            use_current_check: self.use_current_checked,
            builtin_zfs_check: self.switch.zfs_checked,
            builtin_nvidia_open_check: self.builtin_nvidia_open_checked,
            build_debug_check: self.build_debug_checked,
            hz_ticks_combo: self.switch.hz_selected.value().to_string(),
            tickrate_combo: self.tickless.value().to_string(),
            preempt_combo: self.switch.preempt_selected.value().to_string(),
            hugepage_combo: self.hugepage.value().to_string(),
            lto_combo: self.switch.lto_selected.value().to_string(),
            cpu_opt_combo: self.cpu_opt.value().to_string(),
            custom_name_edit: self.custom_name.clone(),
        }
    }

    /// `on_load` (`conf-window.cpp:767-810`): apply a loaded config to the
    /// widgets. Returns true when any combo value was unknown (the
    /// `Config file(%1) is outdated` critical dialog — the checkboxes and
    /// custom name still apply).
    pub fn load_config(&mut self, config: &KernelManagerConfig) -> bool {
        self.hardly_checked = config.hardly_check;
        self.per_gov_checked = config.per_gov_check;
        self.tcp_bbr3_checked = config.tcp_bbr3_check;
        self.nconfig_checked = config.nconfig_check;
        self.xconfig_checked = config.xconfig_check;
        self.localmodcfg_checked = config.localmodcfg_check;
        self.use_current_checked = config.use_current_check;
        self.builtin_nvidia_open_checked = config.builtin_nvidia_open_check;
        self.build_debug_checked = config.build_debug_check;
        self.custom_name = config.custom_name_edit.clone();
        let mut outdated = false;
        match TicklessMode::ALL
            .iter()
            .find(|t| t.value() == config.tickrate_combo)
        {
            Some(t) => self.tickless = *t,
            None => outdated = true,
        }
        match HugepageMode::ALL
            .iter()
            .find(|h| h.value() == config.hugepage_combo)
        {
            Some(h) => self.hugepage = *h,
            None => outdated = true,
        }
        match CpuOptMode::ALL
            .iter()
            .find(|c| c.value() == config.cpu_opt_combo)
        {
            Some(c) => self.cpu_opt = *c,
            None => outdated = true,
        }
        match self
            .switch
            .lto_items
            .iter()
            .find(|l| l.value() == config.lto_combo)
        {
            Some(l) => self.switch.lto_selected = *l,
            None => outdated = true,
        }
        match self
            .switch
            .preempt_items
            .iter()
            .find(|p| p.value() == config.preempt_combo)
        {
            Some(p) => self.switch.preempt_selected = *p,
            None => outdated = true,
        }
        match HzTick::ALL
            .iter()
            .find(|h| h.value() == config.hz_ticks_combo)
        {
            Some(h) => self.switch.hz_selected = *h,
            None => outdated = true,
        }
        self.switch.cachy_config_checked = config.cachy_config_check;
        self.switch.zfs_checked = config.builtin_zfs_check;
        outdated
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

    #[test]
    fn to_config_writes_the_full_18_field_config() {
        let m = ConfigureWindowModel::default();
        let c = m.to_config();
        assert!(c.hardly_check);
        assert!(c.cachy_config_check);
        assert_eq!(c.lto_combo, "thin");
        assert_eq!(c.preempt_combo, "full");
        assert_eq!(c.hz_ticks_combo, "1000");
        assert_eq!(c.tickrate_combo, "full");
        assert_eq!(c.hugepage_combo, "always");
        assert_eq!(c.cpu_opt_combo, "manual");
        assert_eq!(c.custom_name_edit, DEFAULT_CUSTOM_NAME);
        assert!(!c.nconfig_check);
        assert!(!c.per_gov_check);
        assert!(!c.builtin_nvidia_open_check);
        assert!(!c.build_debug_check);
    }

    #[test]
    fn load_config_applies_widgets_and_flags_outdated() {
        let mut m = ConfigureWindowModel::default();
        let mut c = m.to_config();
        c.hardly_check = false;
        c.per_gov_check = true;
        c.lto_combo = "full".to_string();
        c.custom_name_edit = "my-kernel".to_string();
        let outdated = m.load_config(&c);
        assert!(!outdated);
        assert!(!m.hardly_checked);
        assert!(m.per_gov_checked);
        assert_eq!(m.switch.lto_selected, LtoMode::Full);
        assert_eq!(m.custom_name, "my-kernel");

        // an unknown combo value -> the outdated dialog flag
        c.lto_combo = "bogus-lto".to_string();
        assert!(m.load_config(&c));
    }
}
