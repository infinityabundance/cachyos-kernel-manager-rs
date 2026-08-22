//! Build configuration options model.
//!
//! Reconstructed from `oracle/upstream/src/conf-window.cpp` and
//! `oracle/upstream/src/compile_options.json` (revision `6b4a373e`).
//! Protected by courts: `option-transitions/*`, `build-env/*`.
//!
//! The serialized TOML schema lives in the `config` crate; this module is the
//! *semantic* model (value lists, defaults, variant transitions, env var
//! rendering).

//! Option enums are value/label pairs whose variant names ARE the values;
//! their `value()`/`label()`/`ALL` items are self-describing. The UI-facing
//! semantics are documented in docs/COMPATIBILITY.md.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

/// The checkbox → build-variable binding table, in the exact order the
/// oracle emits them (`conf-window.cpp:164-176,428-430`).
pub const CHECKBOX_BINDINGS: &[(&str, &str)] = &[
    ("hardly", "_cc_harder"),
    ("per_gov", "_per_gov"),
    ("tcp_bbr3", "_tcp_bbr3"),
    ("cachy_config", "_cachy_config"),
    ("nconfig", "_makenconfig"),
    ("xconfig", "_makexconfig"),
    ("localmodcfg", "_localmodcfg"),
    ("use_current", "_use_current"),
    ("builtin_zfs", "_build_zfs"),
    ("builtin_nvidia_open", "_build_nvidia_open"),
    ("build_debug", "_build_debug"),
];

/// The kernel variant combo. Index order is the oracle's combo order
/// (`conf-window.cpp:103` + `conf-window.cpp:486-497`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs)] // variants are self-describing; see [`KernelVariant::label`]
pub enum KernelVariant {
    Cachyos,
    Bore,
    Rc,
    Rt,
    Lts,
    Eevdf,
    Bmq,
    Hardened,
    Deckify,
    Server,
}

impl KernelVariant {
    /// All variants in combo order.
    pub const ALL: [KernelVariant; 10] = [
        KernelVariant::Cachyos,
        KernelVariant::Bore,
        KernelVariant::Rc,
        KernelVariant::Rt,
        KernelVariant::Lts,
        KernelVariant::Eevdf,
        KernelVariant::Bmq,
        KernelVariant::Hardened,
        KernelVariant::Deckify,
        KernelVariant::Server,
    ];

    /// Combo index (0-based).
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|v| *v == self)
            .expect("variant in ALL")
    }

    /// `get_kernel_name(index)` — the internal id string.
    pub fn id(self) -> &'static str {
        match self {
            KernelVariant::Cachyos => "cachyos",
            KernelVariant::Bore => "bore",
            KernelVariant::Rc => "rc",
            KernelVariant::Rt => "rt",
            KernelVariant::Lts => "lts",
            KernelVariant::Eevdf => "eevdf",
            KernelVariant::Bmq => "bmq",
            KernelVariant::Hardened => "hardened",
            KernelVariant::Deckify => "deckify",
            KernelVariant::Server => "server",
        }
    }

    /// `get_kernel_name_path(id)` — the PKGBUILD directory name inside the
    /// linux-cachyos repo (`conf-window.cpp:124-148`). Note `rt` maps to
    /// `linux-cachyos-rt-bore`; fallback is `linux-cachyos`.
    pub fn dir_name(self) -> &'static str {
        match self {
            KernelVariant::Cachyos => "linux-cachyos",
            KernelVariant::Bmq => "linux-cachyos-bmq",
            KernelVariant::Bore => "linux-cachyos-bore",
            KernelVariant::Hardened => "linux-cachyos-hardened",
            KernelVariant::Lts => "linux-cachyos-lts",
            KernelVariant::Rc => "linux-cachyos-rc",
            KernelVariant::Rt => "linux-cachyos-rt-bore",
            KernelVariant::Eevdf => "linux-cachyos-eevdf",
            KernelVariant::Deckify => "linux-cachyos-deckify",
            KernelVariant::Server => "linux-cachyos-server",
        }
    }

    /// The English label from `conf-window.cpp:486-497` (source strings for
    /// `tr()`; the i18n crate maps them to catalogs).
    pub fn label(self) -> &'static str {
        match self {
            KernelVariant::Cachyos => "CachyOS default Scheduler (tuned EEVDF)",
            KernelVariant::Bore => "BORE - Burst-Oriented Response Enhancer",
            KernelVariant::Rc => "RC - Release Candidate",
            KernelVariant::Rt => "RT - Realtime kernel",
            KernelVariant::Lts => "LTS - Long-term support kernel",
            KernelVariant::Eevdf => "EEVDF",
            KernelVariant::Bmq => "BMQ (BitMap Queue)",
            KernelVariant::Hardened => "Hardened - Hardened Linux kernel",
            KernelVariant::Deckify => "Deckify - Handheld optimized kernel",
            KernelVariant::Server => "Server - Server optimized kernel",
        }
    }
}

/// HZ tick options, in combo order (values from `conf-window.cpp:104`,
/// labels from `conf-window.cpp:503-511` — note the mixed `HZ`/`Hz` casing
/// is the oracle's own).
///
/// Variants are self-describing (the variant name IS the value); see
/// [`HzTick::value`] for the PKGBUILD variable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum HzTick {
    Hz1000,
    Hz750,
    Hz600,
    Hz500,
    Hz300,
    Hz250,
    Hz100,
}

impl HzTick {
    pub const ALL: [HzTick; 7] = [
        HzTick::Hz1000,
        HzTick::Hz750,
        HzTick::Hz600,
        HzTick::Hz500,
        HzTick::Hz300,
        HzTick::Hz250,
        HzTick::Hz100,
    ];

    pub fn value(self) -> &'static str {
        match self {
            HzTick::Hz1000 => "1000",
            HzTick::Hz750 => "750",
            HzTick::Hz600 => "600",
            HzTick::Hz500 => "500",
            HzTick::Hz300 => "300",
            HzTick::Hz250 => "250",
            HzTick::Hz100 => "100",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HzTick::Hz1000 => "1000HZ",
            HzTick::Hz750 => "750Hz",
            HzTick::Hz600 => "600Hz",
            HzTick::Hz500 => "500Hz",
            HzTick::Hz300 => "300Hz",
            HzTick::Hz250 => "250Hz",
            HzTick::Hz100 => "100Hz",
        }
    }
}

/// Tickless mode options (`full idle periodic`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum TicklessMode {
    Full,
    Idle,
    Periodic,
}

impl TicklessMode {
    pub const ALL: [TicklessMode; 3] = [
        TicklessMode::Full,
        TicklessMode::Idle,
        TicklessMode::Periodic,
    ];

    pub fn value(self) -> &'static str {
        match self {
            TicklessMode::Full => "full",
            TicklessMode::Idle => "idle",
            TicklessMode::Periodic => "periodic",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TicklessMode::Full => "Full",
            TicklessMode::Idle => "Idle",
            TicklessMode::Periodic => "Periodic",
        }
    }
}

/// Preemption options. `Voluntary`/`None` are dynamically present only for
/// the lts/hardened variants (`conf-window.cpp:576-584`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum PreemptMode {
    Full,
    Lazy,
    Voluntary,
    None,
}

impl PreemptMode {
    pub const ALL: [PreemptMode; 4] = [
        PreemptMode::Full,
        PreemptMode::Lazy,
        PreemptMode::Voluntary,
        PreemptMode::None,
    ];

    pub fn value(self) -> &'static str {
        match self {
            PreemptMode::Full => "full",
            PreemptMode::Lazy => "lazy",
            PreemptMode::Voluntary => "voluntary",
            PreemptMode::None => "none",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PreemptMode::Full => "Full",
            PreemptMode::Lazy => "Lazy",
            PreemptMode::Voluntary => "Voluntary",
            PreemptMode::None => "None",
        }
    }
}

/// LTO options. `ThinDist` is dynamically present only for non-lts and
/// non-hardened variants (`conf-window.cpp:564-570`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum LtoMode {
    None,
    Full,
    Thin,
    ThinDist,
}

impl LtoMode {
    pub const ALL: [LtoMode; 4] = [
        LtoMode::None,
        LtoMode::Full,
        LtoMode::Thin,
        LtoMode::ThinDist,
    ];

    pub fn value(self) -> &'static str {
        match self {
            LtoMode::None => "none",
            LtoMode::Full => "full",
            LtoMode::Thin => "thin",
            LtoMode::ThinDist => "thin-dist",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LtoMode::None => "No",
            LtoMode::Full => "Full",
            LtoMode::Thin => "Thin",
            LtoMode::ThinDist => "Thin-dist",
        }
    }
}

/// Transparent hugepage options (`always madvise`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum HugepageMode {
    Always,
    Madvise,
}

impl HugepageMode {
    pub const ALL: [HugepageMode; 2] = [HugepageMode::Always, HugepageMode::Madvise];

    pub fn value(self) -> &'static str {
        match self {
            HugepageMode::Always => "always",
            HugepageMode::Madvise => "madvise",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HugepageMode::Always => "Always",
            HugepageMode::Madvise => "Madvise",
        }
    }
}

/// CPU optimization options. Index 0 (`Manual`) is the "Disabled" combo item
/// and is *omitted* from the environment entirely
/// (`conf-window.cpp:439-442`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum CpuOptMode {
    Manual,
    Native,
    GenericV1,
    GenericV2,
    GenericV3,
    GenericV4,
    Zen4,
}

impl CpuOptMode {
    pub const ALL: [CpuOptMode; 7] = [
        CpuOptMode::Manual,
        CpuOptMode::Native,
        CpuOptMode::GenericV1,
        CpuOptMode::GenericV2,
        CpuOptMode::GenericV3,
        CpuOptMode::GenericV4,
        CpuOptMode::Zen4,
    ];

    pub fn value(self) -> &'static str {
        match self {
            CpuOptMode::Manual => "manual",
            CpuOptMode::Native => "native",
            CpuOptMode::GenericV1 => "generic_v1",
            CpuOptMode::GenericV2 => "generic_v2",
            CpuOptMode::GenericV3 => "generic_v3",
            CpuOptMode::GenericV4 => "generic_v4",
            CpuOptMode::Zen4 => "zen4",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CpuOptMode::Manual => "Disabled",
            CpuOptMode::Native => "Native CPU",
            CpuOptMode::GenericV1 => "Generic / x86_64",
            CpuOptMode::GenericV2 => "x86_64_v2",
            CpuOptMode::GenericV3 => "x86_64_v3",
            CpuOptMode::GenericV4 => "x86_64_v4",
            CpuOptMode::Zen4 => "Zen4",
        }
    }
}

/// Variant-dependent option availability and defaults
/// (`conf-window.cpp:553-602` — the `main_combo_box` change handler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantTransitions {
    /// `thin-dist` available for this variant (not lts, not hardened).
    pub lto_thin_dist: bool,
    /// `Voluntary`/`None` preempt items available (lts or hardened only).
    pub extended_preempt: bool,
    /// LTO default on variant switch: thin for cachyos/rc, none otherwise.
    pub lto_default: LtoMode,
    /// Preempt default: lazy for server, full otherwise.
    pub preempt_default: PreemptMode,
    /// HZ default: 300 for server, 1000 otherwise.
    pub hz_default: HzTick,
    /// cachy_config default: unchecked for server.
    pub cachy_config_default: bool,
    /// builtin_zfs enabled: false for rt (forced unchecked there too).
    pub zfs_enabled: bool,
}

impl KernelVariant {
    /// The variant-switch transition rules, byte-for-byte from the oracle's
    /// change handler.
    pub fn transitions(self) -> VariantTransitions {
        VariantTransitions {
            lto_thin_dist: self != KernelVariant::Lts && self != KernelVariant::Hardened,
            extended_preempt: self == KernelVariant::Hardened || self == KernelVariant::Lts,
            lto_default: if self == KernelVariant::Cachyos || self == KernelVariant::Rc {
                LtoMode::Thin
            } else {
                LtoMode::None
            },
            preempt_default: if self == KernelVariant::Server {
                PreemptMode::Lazy
            } else {
                PreemptMode::Full
            },
            hz_default: if self == KernelVariant::Server {
                HzTick::Hz300
            } else {
                HzTick::Hz1000
            },
            cachy_config_default: self != KernelVariant::Server,
            zfs_enabled: self != KernelVariant::Rt,
        }
    }
}

/// The stateful observable state of the Configure window's option controls
/// after variant switches (`conf-window.cpp:553-602`).
///
/// The oracle's transition handler is STATEFUL: combo item add/remove is
/// count-based (3<->4 lto items, 2<->4 preempt items), and builtin_zfs is
/// force-unchecked when switching TO rt but never re-checked when switching
/// away. This model reproduces that statefulness so the option-transitions
/// court can compare the full control state after arbitrary switch
/// sequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantSwitchState {
    /// The lto combo items (values), in combo order.
    pub lto_items: Vec<LtoMode>,
    /// The selected lto value.
    pub lto_selected: LtoMode,
    /// The preempt combo items (values), in combo order.
    pub preempt_items: Vec<PreemptMode>,
    /// The selected preempt value.
    pub preempt_selected: PreemptMode,
    /// The selected hz value.
    pub hz_selected: HzTick,
    /// The cachy_config checkbox state.
    pub cachy_config_checked: bool,
    /// The builtin_zfs checkbox VALUE (checked or not).
    pub zfs_checked: bool,
    /// The builtin_zfs checkbox ENABLED state (clickable or not).
    pub zfs_enabled: bool,
}

impl Default for VariantSwitchState {
    /// The initial window state (`conf-window.cpp:475-546`): the ctor adds
    /// all four lto items (selected thin), the two preempt items (selected
    /// full), and checks only hardly + cachy_config.
    fn default() -> Self {
        VariantSwitchState {
            lto_items: vec![
                LtoMode::None,
                LtoMode::Full,
                LtoMode::Thin,
                LtoMode::ThinDist,
            ],
            lto_selected: LtoMode::Thin,
            preempt_items: vec![PreemptMode::Full, PreemptMode::Lazy],
            preempt_selected: PreemptMode::Full,
            hz_selected: HzTick::Hz1000,
            cachy_config_checked: true,
            zfs_checked: false,
            zfs_enabled: true,
        }
    }
}

impl VariantSwitchState {
    /// Apply the oracle's `main_combo_box` change handler
    /// (`conf-window.cpp:553-602`) for `variant`. Count-based add/remove and
    /// the rt zfs force-uncheck are reproduced exactly; signal blockers are
    /// irrelevant (no cascading updates happen on the candidate side).
    pub fn switch_to(&mut self, variant: KernelVariant) {
        // thin-dist is not available for lts and hardened
        let has_thin_dist = variant != KernelVariant::Lts && variant != KernelVariant::Hardened;
        if has_thin_dist && self.lto_items.len() == 3 {
            self.lto_items.push(LtoMode::ThinDist);
        } else if !has_thin_dist && self.lto_items.len() == 4 {
            self.lto_items.pop();
        }

        // thin for cachyos/rc, none for others
        self.lto_selected = if variant == KernelVariant::Cachyos || variant == KernelVariant::Rc {
            LtoMode::Thin
        } else {
            LtoMode::None
        };

        // voluntary/none only available for hardened and lts
        let has_extended_preempt =
            variant == KernelVariant::Hardened || variant == KernelVariant::Lts;
        if has_extended_preempt && self.preempt_items.len() == 2 {
            self.preempt_items.push(PreemptMode::Voluntary);
            self.preempt_items.push(PreemptMode::None);
        } else if !has_extended_preempt && self.preempt_items.len() == 4 {
            self.preempt_items.pop();
            self.preempt_items.pop();
        }

        // lazy for server, full for others
        self.preempt_selected = if variant == KernelVariant::Server {
            PreemptMode::Lazy
        } else {
            PreemptMode::Full
        };

        // 300 for server, 1000 for others
        self.hz_selected = if variant == KernelVariant::Server {
            HzTick::Hz300
        } else {
            HzTick::Hz1000
        };

        // unchecked for server, checked for others
        self.cachy_config_checked = variant != KernelVariant::Server;

        // incompatible with realtime kernels: rt disables the checkbox and
        // force-unchecks it; switching away does NOT re-check it
        self.zfs_enabled = variant != KernelVariant::Rt;
        if variant == KernelVariant::Rt {
            self.zfs_checked = false;
        }
    }
}

/// The default custom package name field value
/// (`conf-options-page.ui:53`).
pub const DEFAULT_CUSTOM_NAME: &str = "$pkgbase-custom";

/// Sentinel that disables the `_use_lto_suffix=n` workaround
/// (`conf-window.cpp:446`).
pub const PKGBASE_SENTINEL: &str = "$pkgbase";

/// The complete build configuration as held by the Configure window.
/// Checkbox semantics: enabled = `yes`, disabled = `no` (both are emitted,
/// unlike combo values which are emitted only when non-default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs)] // fields map 1:1 to the UI controls documented in docs/COMPATIBILITY.md
pub struct BuildOptions {
    pub variant: KernelVariant,
    pub hardly: bool,
    pub per_gov: bool,
    pub tcp_bbr3: bool,
    pub cachy_config: bool,
    pub nconfig: bool,
    pub xconfig: bool,
    pub localmodcfg: bool,
    pub use_current: bool,
    pub builtin_zfs: bool,
    pub builtin_nvidia_open: bool,
    pub build_debug: bool,
    pub hz_ticks: HzTick,
    pub tickless: TicklessMode,
    pub preempt: PreemptMode,
    pub hugepage: HugepageMode,
    pub lto: LtoMode,
    pub cpu_opt: CpuOptMode,
    /// Custom `pkgbase`; default `$pkgbase-custom`; sentinel `$pkgbase`.
    pub custom_name: String,
}

impl Default for BuildOptions {
    /// Initial window state (`conf-window.cpp:499-501,541` + variant default
    /// for the initially selected variant, which is index 0 = cachyos).
    fn default() -> Self {
        let v = KernelVariant::Cachyos;
        let t = v.transitions();
        BuildOptions {
            variant: v,
            hardly: true,
            per_gov: false,
            tcp_bbr3: false,
            cachy_config: t.cachy_config_default,
            nconfig: false,
            xconfig: false,
            localmodcfg: false,
            use_current: false,
            // the .ui has no default checked state for builtin_zfs_check, and
            // the ctor sets only hardly + cachy_config (conf-window.cpp:500-501)
            builtin_zfs: false,
            builtin_nvidia_open: false,
            build_debug: false,
            hz_ticks: t.hz_default,
            tickless: TicklessMode::Full,
            preempt: t.preempt_default,
            hugepage: HugepageMode::Always,
            lto: t.lto_default,
            cpu_opt: CpuOptMode::Manual,
            custom_name: DEFAULT_CUSTOM_NAME.to_string(),
        }
    }
}

impl BuildOptions {
    /// The environment variable assignments in the oracle's exact order and
    /// exact rendering (`get_all_set_values`, `conf-window.cpp:421-451`).
    /// Returns `(var, value)` pairs; joining them as `var=value\n` reproduces
    /// the oracle's `all_set_values` string byte-for-byte (including the
    /// trailing newline from `convert_to_var_assign`).
    pub fn env_pairs(&self) -> Vec<(&'static str, String)> {
        let mut out: Vec<(&'static str, String)> = Vec::new();
        let checked = [
            self.hardly,
            self.per_gov,
            self.tcp_bbr3,
            self.cachy_config,
            self.nconfig,
            self.xconfig,
            self.localmodcfg,
            self.use_current,
            self.builtin_zfs,
            self.builtin_nvidia_open,
            self.build_debug,
        ];
        for ((_, var), val) in CHECKBOX_BINDINGS.iter().zip(checked) {
            // convert_to_var_assign_empty_wrapped: enabled -> "yes", else "no"
            out.push((
                var,
                if val {
                    "yes".to_string()
                } else {
                    "no".to_string()
                },
            ));
        }
        out.push(("_HZ_ticks", self.hz_ticks.value().to_string()));
        out.push(("_tickrate", self.tickless.value().to_string()));
        out.push(("_preempt", self.preempt.value().to_string()));
        out.push(("_hugepage", self.hugepage.value().to_string()));
        // option_map: "lto" -> "_use_llvm_lto" (compile_options.json)
        out.push(("_use_llvm_lto", self.lto.value().to_string()));
        if self.cpu_opt != CpuOptMode::Manual {
            out.push(("_processor_opt", self.cpu_opt.value().to_string()));
        }
        // NOTE: workaround PKGBUILD incorrectly working with custom pkgname
        if self.lto != LtoMode::None && self.custom_name != PKGBASE_SENTINEL {
            out.push(("_use_lto_suffix", "n".to_string()));
        }
        out
    }

    /// `all_set_values` string as consumed by the oracle's bash testscripts:
    /// `var=value\n` lines, with a trailing newline.
    pub fn env_string(&self) -> String {
        let mut s = String::new();
        for (var, value) in self.env_pairs() {
            s.push_str(&format!("{var}={value}\n"));
        }
        s
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // test fixtures mutate defaults deliberately
mod tests {
    use super::*;

    #[test]
    fn variant_index_id_dir_label_are_consistent() {
        let ids = [
            "cachyos", "bore", "rc", "rt", "lts", "eevdf", "bmq", "hardened", "deckify", "server",
        ];
        for (i, v) in KernelVariant::ALL.iter().enumerate() {
            assert_eq!(v.index(), i);
            assert_eq!(v.id(), ids[i]);
        }
        assert_eq!(KernelVariant::Rt.dir_name(), "linux-cachyos-rt-bore");
        assert_eq!(KernelVariant::Cachyos.dir_name(), "linux-cachyos");
        assert_eq!(
            KernelVariant::Server.label(),
            "Server - Server optimized kernel"
        );
    }

    #[test]
    fn variant_transitions_match_oracle_rules() {
        let c = KernelVariant::Cachyos.transitions();
        assert!(c.lto_thin_dist);
        assert!(!c.extended_preempt);
        assert_eq!(c.lto_default, LtoMode::Thin);
        assert_eq!(c.preempt_default, PreemptMode::Full);
        assert_eq!(c.hz_default, HzTick::Hz1000);
        assert!(c.cachy_config_default);
        assert!(c.zfs_enabled);

        let lts = KernelVariant::Lts.transitions();
        assert!(!lts.lto_thin_dist);
        assert!(lts.extended_preempt);
        assert_eq!(lts.lto_default, LtoMode::None);

        let hardened = KernelVariant::Hardened.transitions();
        assert!(!hardened.lto_thin_dist);
        assert!(hardened.extended_preempt);

        let server = KernelVariant::Server.transitions();
        assert_eq!(server.preempt_default, PreemptMode::Lazy);
        assert_eq!(server.hz_default, HzTick::Hz300);
        assert!(!server.cachy_config_default);

        let rt = KernelVariant::Rt.transitions();
        assert!(!rt.zfs_enabled);

        let rc = KernelVariant::Rc.transitions();
        assert_eq!(rc.lto_default, LtoMode::Thin);
    }

    #[test]
    fn env_rendering_matches_oracle_order_and_values() {
        let opts = BuildOptions::default();
        let pairs = opts.env_pairs();
        let vars: Vec<&str> = pairs.iter().map(|(v, _)| *v).collect();
        assert_eq!(
            vars,
            vec![
                "_cc_harder",
                "_per_gov",
                "_tcp_bbr3",
                "_cachy_config",
                "_makenconfig",
                "_makexconfig",
                "_localmodcfg",
                "_use_current",
                "_build_zfs",
                "_build_nvidia_open",
                "_build_debug",
                "_HZ_ticks",
                "_tickrate",
                "_preempt",
                "_hugepage",
                "_use_llvm_lto",
                "_use_lto_suffix"
            ]
        );
        // defaults: hardly=yes, cachy_config=yes, others no
        let get = |v: &str| pairs.iter().find(|(var, _)| *var == v).unwrap().1.clone();
        assert_eq!(get("_cc_harder"), "yes");
        assert_eq!(get("_per_gov"), "no");
        assert_eq!(get("_cachy_config"), "yes");
        assert_eq!(get("_HZ_ticks"), "1000");
        assert_eq!(get("_tickrate"), "full");
        assert_eq!(get("_preempt"), "full");
        assert_eq!(get("_hugepage"), "always");
        assert_eq!(get("_use_llvm_lto"), "thin");
        // cpu_opt manual -> omitted
        assert!(pairs.iter().all(|(v, _)| *v != "_processor_opt"));
        // lto != none && custom name != $pkgbase -> _use_lto_suffix=n
        assert_eq!(get("_use_lto_suffix"), "n");
    }

    #[test]
    fn env_rendering_cpu_opt_and_lto_workaround() {
        let mut opts = BuildOptions::default();
        opts.cpu_opt = CpuOptMode::Zen4;
        opts.lto = LtoMode::None;
        opts.custom_name = PKGBASE_SENTINEL.to_string();
        // recompute pairs after every mutation (env_pairs is a snapshot)
        let get = |o: &BuildOptions, v: &str| {
            o.env_pairs()
                .iter()
                .find(|(var, _)| *var == v)
                .map(|(_, val)| val.clone())
        };
        assert_eq!(get(&opts, "_processor_opt"), Some("zen4".to_string()));
        // lto none -> no suffix workaround even with custom name
        assert_eq!(get(&opts, "_use_lto_suffix"), None);
        // sentinel custom name -> no suffix workaround
        opts.custom_name = DEFAULT_CUSTOM_NAME.to_string();
        assert_eq!(get(&opts, "_use_lto_suffix"), None);
        opts.lto = LtoMode::Thin;
        assert_eq!(get(&opts, "_use_lto_suffix"), Some("n".to_string()));
    }

    #[test]
    fn env_string_has_trailing_newline_like_oracle() {
        let s = BuildOptions::default().env_string();
        assert!(s.ends_with('\n'));
        assert!(s.starts_with("_cc_harder=yes\n"));
        assert!(s.contains("_use_llvm_lto=thin\n"));
    }

    #[test]
    fn option_value_lists_match_conf_window() {
        assert_eq!(
            HzTick::ALL.map(|h| h.value()).as_slice(),
            ["1000", "750", "600", "500", "300", "250", "100"]
        );
        assert_eq!(
            TicklessMode::ALL.map(|t| t.value()).as_slice(),
            ["full", "idle", "periodic"]
        );
        assert_eq!(
            PreemptMode::ALL.map(|p| p.value()).as_slice(),
            ["full", "lazy", "voluntary", "none"]
        );
        assert_eq!(
            LtoMode::ALL.map(|l| l.value()).as_slice(),
            ["none", "full", "thin", "thin-dist"]
        );
        assert_eq!(
            HugepageMode::ALL.map(|h| h.value()).as_slice(),
            ["always", "madvise"]
        );
        assert_eq!(
            CpuOptMode::ALL.map(|c| c.value()).as_slice(),
            [
                "manual",
                "native",
                "generic_v1",
                "generic_v2",
                "generic_v3",
                "generic_v4",
                "zen4"
            ]
        );
    }
}
