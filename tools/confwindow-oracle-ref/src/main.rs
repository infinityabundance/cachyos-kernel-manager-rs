//! Reference harness reproducing the ORACLE's Configure-window semantics
//! (`oracle/upstream/src/conf-window.cpp`, revision `6b4a373e`)
//! byte-for-byte:
//!
//! - the ctor (`conf-window.cpp:475-546`): variant labels, hardly +
//!   cachy_config checked, the combo lists, LTO thin initially selected;
//! - the variant switch handler (`conf-window.cpp:553-602`): thin-dist
//!   availability (not lts/hardened), lto default (thin for cachyos/rc else
//!   none), preempt extension (Voluntary/None for lts/hardened), preempt
//!   default (lazy for server else full), hz default (300 server else 1000),
//!   cachy_config default (unchecked for server), builtin_zfs (rt disables
//!   and force-unchecks);
//! - `reset_patches_data_tab` (`conf-window.cpp:458-473`): the source-array
//!   probe filtered to `.patch`; the list ops (`615-686`);
//! - the `_use_lto_suffix=n` condition (`conf-window.cpp:446`);
//! - `on_save`'s UI state (`conf-window.cpp:743-755`).
//!
//! Input: the shared corpus (`cachyos-km-configure-window-corpus-v1`).
//! Output: the model JSON. This tool is court evidence infrastructure,
//! never shipped.

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

const VARIANT_LABELS: [&str; 10] = [
    "CachyOS default Scheduler (tuned EEVDF)",
    "BORE - Burst-Oriented Response Enhancer",
    "RC - Release Candidate",
    "RT - Realtime kernel",
    "LTS - Long-term support kernel",
    "EEVDF",
    "BMQ (BitMap Queue)",
    "Hardened - Hardened Linux kernel",
    "Deckify - Handheld optimized kernel",
    "Server - Server optimized kernel",
];

const PKGBASE_SENTINEL: &str = "$pkgbase";
const DEFAULT_CUSTOM_NAME: &str = "$pkgbase-custom";

fn variant_id(name: &str) -> Option<&'static str> {
    match name {
        "cachyos" => Some("cachyos"),
        "bore" => Some("bore"),
        "rc" => Some("rc"),
        "rt" => Some("rt"),
        "lts" => Some("lts"),
        "eevdf" => Some("eevdf"),
        "bmq" => Some("bmq"),
        "hardened" => Some("hardened"),
        "deckify" => Some("deckify"),
        "server" => Some("server"),
        _ => None,
    }
}

fn variant_label(id: &str) -> &'static str {
    match id {
        "cachyos" => VARIANT_LABELS[0],
        "bore" => VARIANT_LABELS[1],
        "rc" => VARIANT_LABELS[2],
        "rt" => VARIANT_LABELS[3],
        "lts" => VARIANT_LABELS[4],
        "eevdf" => VARIANT_LABELS[5],
        "bmq" => VARIANT_LABELS[6],
        "hardened" => VARIANT_LABELS[7],
        "deckify" => VARIANT_LABELS[8],
        "server" => VARIANT_LABELS[9],
        _ => VARIANT_LABELS[0],
    }
}

/// The window state (the model's projection).
#[derive(Debug, Clone)]
struct State {
    variant: &'static str,
    hardly_checked: bool,
    cachy_config_checked: bool,
    lto_items: Vec<&'static str>,
    lto_selected: &'static str,
    preempt_items: Vec<&'static str>,
    preempt_selected: &'static str,
    hz_selected: &'static str,
    zfs_enabled: bool,
    zfs_checked: bool,
    patches: Vec<String>,
    custom_name: String,
}

impl Default for State {
    /// The ctor (`conf-window.cpp:475-546`).
    fn default() -> Self {
        State {
            variant: "cachyos",
            hardly_checked: true,
            cachy_config_checked: true,
            lto_items: vec!["none", "full", "thin", "thin-dist"],
            lto_selected: "thin",
            preempt_items: vec!["full", "lazy"],
            preempt_selected: "full",
            hz_selected: "1000",
            zfs_enabled: true,
            zfs_checked: false,
            patches: Vec::new(),
            custom_name: DEFAULT_CUSTOM_NAME.to_string(),
        }
    }
}

impl State {
    /// The `main_combo_box` handler (`conf-window.cpp:553-602`).
    fn switch_to(&mut self, variant: &'static str) {
        self.variant = variant;
        // thin-dist not for lts/hardened (count-based)
        let has_thin_dist = variant != "lts" && variant != "hardened";
        if has_thin_dist && self.lto_items.len() == 3 {
            self.lto_items.push("thin-dist");
        } else if !has_thin_dist && self.lto_items.len() == 4 {
            self.lto_items.pop();
        }
        // thin for cachyos/rc else none
        self.lto_selected = if variant == "cachyos" || variant == "rc" {
            "thin"
        } else {
            "none"
        };
        // voluntary/none for hardened/lts
        let has_extended = variant == "hardened" || variant == "lts";
        if has_extended && self.preempt_items.len() == 2 {
            self.preempt_items.push("voluntary");
            self.preempt_items.push("none");
        } else if !has_extended && self.preempt_items.len() == 4 {
            self.preempt_items.pop();
            self.preempt_items.pop();
        }
        // lazy for server else full
        self.preempt_selected = if variant == "server" { "lazy" } else { "full" };
        // 300 for server else 1000
        self.hz_selected = if variant == "server" { "300" } else { "1000" };
        // unchecked for server
        self.cachy_config_checked = variant != "server";
        // rt disables zfs + force-unchecks
        self.zfs_enabled = variant != "rt";
        if variant == "rt" {
            self.zfs_checked = false;
        }
    }

    /// `reset_patches_data_tab` (`conf-window.cpp:458-473`).
    fn reset_patches(&mut self, source_array: &[String]) {
        self.patches = source_array
            .iter()
            .filter(|item| item.ends_with(".patch"))
            .cloned()
            .collect();
    }

    fn use_lto_suffix(&self) -> bool {
        self.lto_selected != "none" && self.custom_name != PKGBASE_SENTINEL
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
enum PatchOp {
    Reset { source_array: Vec<String> },
    AddLocal { files: Vec<String> },
    AddRemote { url: String },
    Remove { index: usize },
    MoveUp { index: usize },
    MoveDown { index: usize },
}

#[derive(Debug, Deserialize)]
struct SwitchStep {
    variant: String,
    #[serde(default)]
    source_array: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Corpus {
    #[serde(default)]
    variant: String,
    #[serde(default)]
    switches: Vec<SwitchStep>,
    #[serde(default)]
    patch_ops: Vec<PatchOp>,
    #[serde(default)]
    custom_name: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: confwindow-oracle-ref parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: confwindow-oracle-ref parse <corpus.json>");
        return ExitCode::from(2);
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let corpus: Corpus = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut state = State::default();
    if let Some(v) = variant_id(&corpus.variant) {
        state.switch_to(v);
    }
    if let Some(name) = &corpus.custom_name {
        state.custom_name = name.clone();
    }
    for step in &corpus.switches {
        let Some(v) = variant_id(&step.variant) else {
            eprintln!("unknown variant: {:?}", step.variant);
            return ExitCode::FAILURE;
        };
        state.switch_to(v);
        state.reset_patches(&step.source_array);
    }
    for op in &corpus.patch_ops {
        match op {
            PatchOp::Reset { source_array } => state.reset_patches(source_array),
            PatchOp::AddLocal { files } => {
                for f in files {
                    state.patches.push(format!("file://{f}"));
                }
            }
            PatchOp::AddRemote { url } => state.patches.push(url.clone()),
            PatchOp::Remove { index } => {
                if *index < state.patches.len() {
                    state.patches.remove(*index);
                }
            }
            PatchOp::MoveUp { index } => {
                if *index > 0 && *index < state.patches.len() {
                    state.patches.swap(*index, *index - 1);
                }
            }
            PatchOp::MoveDown { index } => {
                if *index + 1 < state.patches.len() {
                    state.patches.swap(*index, *index + 1);
                }
            }
        }
    }
    let payload = json!({
        "schema": "cachyos-km-configure-window-v1",
        "variant": state.variant,
        "variant_label": variant_label(state.variant),
        "hardly_checked": state.hardly_checked,
        "cachy_config_checked": state.cachy_config_checked,
        "lto_items": state.lto_items,
        "lto_selected": state.lto_selected,
        "preempt_items": state.preempt_items,
        "preempt_selected": state.preempt_selected,
        "hz_selected": state.hz_selected,
        "zfs_enabled": state.zfs_enabled,
        "zfs_checked": state.zfs_checked,
        "patches": state.patches,
        "custom_name": state.custom_name,
        "use_lto_suffix": state.use_lto_suffix(),
        "save": {
            "variant": state.variant,
            "hardly": state.hardly_checked,
            "cachy_config": state.cachy_config_checked,
            "lto": state.lto_selected,
            "preempt": state.preempt_selected,
            "hz_ticks": state.hz_selected,
            "custom_name": state.custom_name,
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
