//! Reference harness reproducing the ORACLE's Configure-window
//! variant-switch transitions (`conf-window.cpp:553-602`, revision
//! `6b4a373e`) byte-for-byte, INCLUDING the stateful parts: count-based lto
//! item add/remove (3<->4), count-based preempt item add/remove (2<->4), and
//! the rt force-uncheck of builtin_zfs (which is never re-checked on
//! switching away).
//!
//! Input: a switch sequence `{"switches": ["lts", "server", ...]}` starting
//! from the constructor initial state (`conf-window.cpp:475-546`: all four
//! lto items + thin selected, two preempt items + full selected, hz=1000,
//! cachy_config checked, builtin_zfs unchecked+enabled).
//! Output: the state after EACH switch, as a JSON array (the initial state
//! first), values in the PKGBUILD value form.
//!
//! Usage: options-oracle-ref parse <sequence.json>
//! This tool is court evidence infrastructure, never shipped.

use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Sequence {
    switches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lto {
    None,
    Full,
    Thin,
    ThinDist,
}
impl Lto {
    fn value(&self) -> &'static str {
        match self {
            Lto::None => "none",
            Lto::Full => "full",
            Lto::Thin => "thin",
            Lto::ThinDist => "thin-dist",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Preempt {
    Full,
    Lazy,
    Voluntary,
    None,
}
impl Preempt {
    fn value(&self) -> &'static str {
        match self {
            Preempt::Full => "full",
            Preempt::Lazy => "lazy",
            Preempt::Voluntary => "voluntary",
            Preempt::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hz {
    Hz1000,
    Hz300,
}
impl Hz {
    fn value(&self) -> &'static str {
        match self {
            Hz::Hz1000 => "1000",
            Hz::Hz300 => "300",
        }
    }
}

#[derive(Debug, Clone)]
struct State {
    lto_items: Vec<Lto>,
    lto_selected: Lto,
    preempt_items: Vec<Preempt>,
    preempt_selected: Preempt,
    hz_selected: Hz,
    cachy_config_checked: bool,
    zfs_checked: bool,
    zfs_enabled: bool,
}

impl State {
    /// The constructor initial state (`conf-window.cpp:475-546`).
    fn initial() -> State {
        State {
            lto_items: vec![Lto::None, Lto::Full, Lto::Thin, Lto::ThinDist],
            lto_selected: Lto::Thin,
            preempt_items: vec![Preempt::Full, Preempt::Lazy],
            preempt_selected: Preempt::Full,
            hz_selected: Hz::Hz1000,
            cachy_config_checked: true,
            zfs_checked: false,
            zfs_enabled: true,
        }
    }

    fn to_json(&self, variant: &str) -> serde_json::Value {
        json!({
            "variant": variant,
            "lto_items": self.lto_items.iter().map(|l| l.value()).collect::<Vec<_>>(),
            "lto_selected": self.lto_selected.value(),
            "preempt_items": self.preempt_items.iter().map(|p| p.value()).collect::<Vec<_>>(),
            "preempt_selected": self.preempt_selected.value(),
            "hz_selected": self.hz_selected.value(),
            "cachy_config_checked": self.cachy_config_checked,
            "zfs_checked": self.zfs_checked,
            "zfs_enabled": self.zfs_enabled,
        })
    }

    /// The oracle's `main_combo_box` change handler (`conf-window.cpp:553-602`).
    fn switch_to(&mut self, kernel_name: &str) {
        let has_thin_dist = kernel_name != "lts" && kernel_name != "hardened";
        if has_thin_dist && self.lto_items.len() == 3 {
            self.lto_items.push(Lto::ThinDist);
        } else if !has_thin_dist && self.lto_items.len() == 4 {
            self.lto_items.pop();
        }

        let lto_thin_default = kernel_name == "cachyos" || kernel_name == "rc";
        self.lto_selected = if lto_thin_default {
            Lto::Thin
        } else {
            Lto::None
        };

        let has_extended_preempt = kernel_name == "hardened" || kernel_name == "lts";
        if has_extended_preempt && self.preempt_items.len() == 2 {
            self.preempt_items.push(Preempt::Voluntary);
            self.preempt_items.push(Preempt::None);
        } else if !has_extended_preempt && self.preempt_items.len() == 4 {
            self.preempt_items.pop();
            self.preempt_items.pop();
        }

        self.preempt_selected = if kernel_name == "server" {
            Preempt::Lazy
        } else {
            Preempt::Full
        };

        self.hz_selected = if kernel_name == "server" {
            Hz::Hz300
        } else {
            Hz::Hz1000
        };

        self.cachy_config_checked = kernel_name != "server";

        self.zfs_enabled = kernel_name != "rt";
        if kernel_name == "rt" {
            self.zfs_checked = false;
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let content = match args.as_slice() {
        [cmd, path] if cmd == "parse" => match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("usage: options-oracle-ref parse <sequence.json>");
            return ExitCode::from(2);
        }
    };
    let seq: Sequence = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut state = State::initial();
    let mut out = vec![state.to_json("initial")];
    for kernel in &seq.switches {
        state.switch_to(kernel);
        out.push(state.to_json(kernel));
    }
    println!("{}", serde_json::to_string_pretty(&out).expect("serialize"));
    ExitCode::SUCCESS
}
