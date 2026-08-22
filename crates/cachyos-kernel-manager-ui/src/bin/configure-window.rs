//! `cachyos-kernel-manager-confwindow` — candidate witness for the
//! `ui/configure-window-semantics` court. Reads the corpus (the initial
//! state, variant switches, source-array probes, patch-list operations) and
//! renders the candidate's REAL Configure-window model
//! (`crates/cachyos-kernel-manager-ui/src/configure_window.rs`).
//!
//! Usage: cachyos-kernel-manager-confwindow parse <corpus.json>

use cachyos_kernel_manager_core::options::KernelVariant;
use cachyos_kernel_manager_ui::configure_window::ConfigureWindowModel;
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

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
struct Corpus {
    /// The initial variant (defaults to cachyos).
    #[serde(default)]
    variant: String,
    /// Variant switches, applied in order (with a source array per switch).
    #[serde(default)]
    switches: Vec<SwitchStep>,
    /// Patch-list operations, applied in order.
    #[serde(default)]
    patch_ops: Vec<PatchOp>,
    #[serde(default)]
    custom_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SwitchStep {
    variant: String,
    #[serde(default)]
    source_array: Vec<String>,
}

fn parse_variant(name: &str) -> Option<KernelVariant> {
    KernelVariant::ALL.iter().copied().find(|v| v.id() == name)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: cachyos-kernel-manager-confwindow parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: cachyos-kernel-manager-confwindow parse <corpus.json>");
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
    let mut model = ConfigureWindowModel::default();
    if let Some(variant) = parse_variant(&corpus.variant) {
        model.on_variant_changed(variant, &[]);
    }
    if let Some(name) = &corpus.custom_name {
        model.custom_name = name.clone();
    }
    for step in &corpus.switches {
        let Some(variant) = parse_variant(&step.variant) else {
            eprintln!("unknown variant: {:?}", step.variant);
            return ExitCode::FAILURE;
        };
        model.on_variant_changed(variant, &step.source_array);
    }
    for op in &corpus.patch_ops {
        match op {
            PatchOp::Reset { source_array } => model.reset_patches(source_array),
            PatchOp::AddLocal { files } => model.add_local_patches(files),
            PatchOp::AddRemote { url } => model.add_remote_patch(url.clone()),
            PatchOp::Remove { index } => model.remove_patch(*index),
            PatchOp::MoveUp { index } => model.move_up(*index),
            PatchOp::MoveDown { index } => model.move_down(*index),
        }
    }
    let save = model.save_ui_state();
    let payload = json!({
        "schema": "cachyos-km-configure-window-v1",
        "variant": model.variant.id(),
        "variant_label": model.variant_label,
        "hardly_checked": model.hardly_checked,
        "cachy_config_checked": model.switch.cachy_config_checked,
        "lto_items": model.switch.lto_items.iter().map(|l| l.value()).collect::<Vec<_>>(),
        "lto_selected": model.switch.lto_selected.value(),
        "preempt_items": model.switch.preempt_items.iter().map(|p| p.value()).collect::<Vec<_>>(),
        "preempt_selected": model.switch.preempt_selected.value(),
        "hz_selected": model.switch.hz_selected.value(),
        "zfs_enabled": model.switch.zfs_enabled,
        "zfs_checked": model.switch.zfs_checked,
        "patches": model.patches,
        "custom_name": model.custom_name,
        "use_lto_suffix": model.use_lto_suffix(),
        "save": {
            "variant": save.variant.id(),
            "hardly": save.hardly,
            "cachy_config": save.cachy_config,
            "lto": save.lto.value(),
            "preempt": save.preempt.value(),
            "hz_ticks": save.hz_ticks.value(),
            "custom_name": save.custom_name,
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
