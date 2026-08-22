//! The `scx_loader` configuration model + mode/flags semantics
//! (`scx_loader 1.0.9` `src/lib.rs` + `src/config.rs`, checksum-pinned by
//! `oracle/scx-authority/SCX-AUTHORITY.md`).
//!
//! Reconstructed byte-for-byte: `SupportedSched`, `SchedMode`, the default
//! per-mode flag matrix, `get_scx_flags_for_mode` (config entry first,
//! hardcoded-default fallback), and the config file shape (`default_sched`,
//! `default_mode`, `scheds`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `SupportedSched` (`scx_loader/src/lib.rs:22-31`) — the four schedulers
/// the loader knows; D-Bus signature `"s"` (`#[zvariant(signature = "s")]`,
/// Type only — exactly like the authority, which derives Type but NOT
/// Value/OwnedValue on this enum).
#[cfg_attr(feature = "dbus", derive(zvariant::Type))]
#[cfg_attr(feature = "dbus", zvariant(signature = "s"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SupportedSched {
    #[serde(rename = "scx_bpfland")]
    Bpfland,
    #[serde(rename = "scx_rusty")]
    Rusty,
    #[serde(rename = "scx_lavd")]
    Lavd,
    #[serde(rename = "scx_flash")]
    Flash,
}

impl SupportedSched {
    /// All four schedulers, in the crate's declaration order.
    pub const ALL: [SupportedSched; 4] = [
        SupportedSched::Bpfland,
        SupportedSched::Rusty,
        SupportedSched::Lavd,
        SupportedSched::Flash,
    ];

    /// The loader-facing name (`From<SupportedSched> for &str`,
    /// `scx_loader/src/lib.rs:68-78`).
    pub fn name(&self) -> &'static str {
        match self {
            SupportedSched::Bpfland => "scx_bpfland",
            SupportedSched::Rusty => "scx_rusty",
            SupportedSched::Lavd => "scx_lavd",
            SupportedSched::Flash => "scx_flash",
        }
    }
}

/// `FromStr` (`scx_loader/src/lib.rs:47-58`): unknown names are errors.
impl std::str::FromStr for SupportedSched {
    type Err = String;
    fn from_str(scx_name: &str) -> Result<SupportedSched, String> {
        match scx_name {
            "scx_bpfland" => Ok(SupportedSched::Bpfland),
            "scx_rusty" => Ok(SupportedSched::Rusty),
            "scx_lavd" => Ok(SupportedSched::Lavd),
            "scx_flash" => Ok(SupportedSched::Flash),
            _ => Err(format!("{scx_name} is not supported")),
        }
    }
}

/// `SchedMode` (`scx_loader/src/lib.rs:34-46`) — the five preset profiles.
/// A fieldless enum with explicit discriminants and NO `repr`, so zvariant
/// encodes it as u32 (`"u"` — the `scheduler_mode` property signature).
#[cfg_attr(
    feature = "dbus",
    derive(zvariant::Type, zvariant::Value, zvariant::OwnedValue)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum SchedMode {
    /// Default values for the scheduler
    Auto = 0,
    /// Applies flags for better gaming experience
    Gaming = 1,
    /// Applies flags for lower power usage
    PowerSave = 2,
    /// Starts scheduler in low latency mode
    LowLatency = 3,
    /// Starts scheduler in server-oriented mode
    Server = 4,
}

impl SchedMode {
    /// All five modes, in discriminant order.
    pub const ALL: [SchedMode; 5] = [
        SchedMode::Auto,
        SchedMode::Gaming,
        SchedMode::PowerSave,
        SchedMode::LowLatency,
        SchedMode::Server,
    ];

    /// The raw discriminant (0..=4).
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// The profile-combo label (`Auto`/`Gaming`/`Powersave`/`Lowlatency`/
    /// `Server` — the UI strings, `schedext-window-internal.cpp:113-115`).
    pub fn label(&self) -> &'static str {
        match self {
            SchedMode::Auto => "Auto",
            SchedMode::Gaming => "Gaming",
            SchedMode::PowerSave => "Powersave",
            SchedMode::LowLatency => "Lowlatency",
            SchedMode::Server => "Server",
        }
    }

    /// The Rust Debug rendering the oracle's println uses
    /// (`{scx_mode:?}` in `scx_loader_config.rs`).
    pub fn debug(&self) -> &'static str {
        match self {
            SchedMode::Auto => "Auto",
            SchedMode::Gaming => "Gaming",
            SchedMode::PowerSave => "PowerSave",
            SchedMode::LowLatency => "LowLatency",
            SchedMode::Server => "Server",
        }
    }
}

/// `convert_from_raw_mode` (`scx_loader_config.rs`): 0..=4 → the mode,
/// anything else is an error ("SchedMode with such value doesn't exist").
pub fn mode_from_raw(raw: u8) -> Result<SchedMode, String> {
    match raw {
        0 => Ok(SchedMode::Auto),
        1 => Ok(SchedMode::Gaming),
        2 => Ok(SchedMode::PowerSave),
        3 => Ok(SchedMode::LowLatency),
        4 => Ok(SchedMode::Server),
        _ => Err("SchedMode with such value doesn't exist".to_string()),
    }
}

/// `get_scx_mode_from_str` (`schedext-window-internal.cpp:93-103`): the
/// profile-combo text → mode; anything else is `Auto`.
pub fn mode_from_label(label: &str) -> SchedMode {
    match label {
        "Gaming" => SchedMode::Gaming,
        "Lowlatency" => SchedMode::LowLatency,
        "Powersave" => SchedMode::PowerSave,
        "Server" => SchedMode::Server,
        _ => SchedMode::Auto,
    }
}

/// The per-scheduler per-mode flag lists (`scx_loader/src/config.rs:25-34`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedFlags {
    pub auto_mode: Option<Vec<String>>,
    pub gaming_mode: Option<Vec<String>>,
    pub lowlatency_mode: Option<Vec<String>>,
    pub powersave_mode: Option<Vec<String>>,
    pub server_mode: Option<Vec<String>>,
}

/// `Config` (`scx_loader/src/config.rs:21-26`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedConfig {
    pub default_sched: Option<SupportedSched>,
    pub default_mode: Option<SchedMode>,
    pub scheds: BTreeMap<String, SchedFlags>,
}

/// The hardcoded default flags matrix
/// (`get_default_scx_flags_for_mode`, `scx_loader/src/config.rs:169-189`).
pub fn default_flags(sched: SupportedSched, mode: SchedMode) -> Vec<&'static str> {
    match sched {
        SupportedSched::Bpfland => match mode {
            SchedMode::Gaming => vec!["-m", "performance"],
            SchedMode::LowLatency => {
                vec!["-s", "5000", "-S", "500", "-l", "5000", "-m", "performance"]
            }
            SchedMode::PowerSave => vec!["-m", "powersave"],
            SchedMode::Server => vec!["-p"],
            SchedMode::Auto => vec![],
        },
        SupportedSched::Lavd => match mode {
            SchedMode::Gaming | SchedMode::LowLatency => vec!["--performance"],
            SchedMode::PowerSave => vec!["--powersave"],
            // NOTE: potentially adding --auto in future
            SchedMode::Server | SchedMode::Auto => vec![],
        },
        // scx_rusty doesn't support any of these modes
        SupportedSched::Rusty => vec![],
        // scx_flash doesn't support any of these modes
        SupportedSched::Flash => vec![],
    }
}

/// `extract_scx_flags_from_config` (`scx_loader/src/config.rs:118-130`).
pub fn flags_field<'a>(flags: &'a SchedFlags, mode: &SchedMode) -> Option<&'a Option<Vec<String>>> {
    match mode {
        SchedMode::Gaming => Some(&flags.gaming_mode),
        SchedMode::LowLatency => Some(&flags.lowlatency_mode),
        SchedMode::PowerSave => Some(&flags.powersave_mode),
        SchedMode::Server => Some(&flags.server_mode),
        SchedMode::Auto => Some(&flags.auto_mode),
    }
}

/// `get_scx_flags_for_mode` (`scx_loader/src/config.rs:101-116`): the
/// config entry's mode field first, else the hardcoded defaults.
pub fn flags_for_mode(config: &SchedConfig, sched: SupportedSched, mode: SchedMode) -> Vec<String> {
    let name = sched.name();
    if let Some(sched_config) = config.scheds.get(name) {
        match flags_field(sched_config, &mode) {
            Some(Some(field)) => field.clone(),
            _ => default_flags(sched, mode)
                .into_iter()
                .map(String::from)
                .collect(),
        }
    } else {
        default_flags(sched, mode)
            .into_iter()
            .map(String::from)
            .collect()
    }
}

/// `get_default_config` (`scx_loader/src/config.rs:75-98`): no default
/// scheduler, Auto mode, and every scheduler with its default flags.
pub fn default_config() -> SchedConfig {
    let mut scheds = BTreeMap::new();
    for sched in SupportedSched::ALL {
        let mut flags = SchedFlags::default();
        for mode in SchedMode::ALL {
            let flags_vec = default_flags(sched, mode)
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            match mode {
                SchedMode::Auto => flags.auto_mode = Some(flags_vec),
                SchedMode::Gaming => flags.gaming_mode = Some(flags_vec),
                SchedMode::PowerSave => flags.powersave_mode = Some(flags_vec),
                SchedMode::LowLatency => flags.lowlatency_mode = Some(flags_vec),
                SchedMode::Server => flags.server_mode = Some(flags_vec),
            }
        }
        scheds.insert(sched.name().to_string(), flags);
    }
    SchedConfig {
        default_sched: None,
        default_mode: Some(SchedMode::Auto),
        scheds,
    }
}

/// `init_config` (`scx_loader_config.rs`): parse the config file when it
/// exists, else the default config.
pub fn init_config(config_path: &str, content: Option<&str>) -> Result<SchedConfig, String> {
    let _ = config_path;
    match content {
        Some(content) => parse_config(content),
        None => Ok(default_config()),
    }
}

/// `parse_config_content` (`scx_loader/src/config.rs:67-73`): an empty file
/// is an error ("The config file is empty!").
pub fn parse_config(content: &str) -> Result<SchedConfig, String> {
    if content.is_empty() {
        return Err("The config file is empty!".to_string());
    }
    toml::from_str(content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_sched_names_and_parse() {
        assert_eq!(SupportedSched::Bpfland.name(), "scx_bpfland");
        assert_eq!(
            "scx_rusty".parse::<SupportedSched>(),
            Ok(SupportedSched::Rusty)
        );
        assert_eq!(
            "scx_unknown".parse::<SupportedSched>(),
            Err("scx_unknown is not supported".to_string())
        );
    }

    #[test]
    fn mode_label_and_raw_roundtrip() {
        assert_eq!(mode_from_label("Gaming"), SchedMode::Gaming);
        assert_eq!(mode_from_label("Lowlatency"), SchedMode::LowLatency);
        assert_eq!(mode_from_label("Powersave"), SchedMode::PowerSave);
        assert_eq!(mode_from_label("Server"), SchedMode::Server);
        assert_eq!(mode_from_label("anything-else"), SchedMode::Auto);
        assert_eq!(mode_from_label("Auto"), SchedMode::Auto);
        assert_eq!(mode_from_raw(4), Ok(SchedMode::Server));
        assert!(mode_from_raw(5).is_err());
        assert_eq!(SchedMode::Auto.as_u8(), 0);
        assert_eq!(SchedMode::Server.as_u8(), 4);
    }

    #[test]
    fn default_flags_matrix_matches_authority() {
        assert_eq!(
            default_flags(SupportedSched::Bpfland, SchedMode::Gaming),
            vec!["-m", "performance"]
        );
        assert_eq!(
            default_flags(SupportedSched::Bpfland, SchedMode::LowLatency),
            vec!["-s", "5000", "-S", "500", "-l", "5000", "-m", "performance"]
        );
        assert_eq!(
            default_flags(SupportedSched::Bpfland, SchedMode::PowerSave),
            vec!["-m", "powersave"]
        );
        assert_eq!(
            default_flags(SupportedSched::Bpfland, SchedMode::Server),
            vec!["-p"]
        );
        assert!(default_flags(SupportedSched::Bpfland, SchedMode::Auto).is_empty());
        assert_eq!(
            default_flags(SupportedSched::Lavd, SchedMode::Gaming),
            vec!["--performance"]
        );
        assert_eq!(
            default_flags(SupportedSched::Lavd, SchedMode::LowLatency),
            vec!["--performance"]
        );
        assert_eq!(
            default_flags(SupportedSched::Lavd, SchedMode::PowerSave),
            vec!["--powersave"]
        );
        assert!(default_flags(SupportedSched::Lavd, SchedMode::Server).is_empty());
        assert!(default_flags(SupportedSched::Rusty, SchedMode::Gaming).is_empty());
        assert!(default_flags(SupportedSched::Flash, SchedMode::Server).is_empty());
    }

    #[test]
    fn config_flags_override_and_fallback() {
        let mut config = default_config();
        // override bpfland Gaming flags in the config entry
        let bpfland = config.scheds.get_mut("scx_bpfland").unwrap();
        bpfland.gaming_mode = Some(vec!["--custom".into()]);
        assert_eq!(
            flags_for_mode(&config, SupportedSched::Bpfland, SchedMode::Gaming),
            vec!["--custom"]
        );
        // an entry with the field absent falls back to the hardcoded default
        let mut sparse = SchedConfig::default();
        sparse
            .scheds
            .insert("scx_bpfland".into(), SchedFlags::default());
        assert_eq!(
            flags_for_mode(&sparse, SupportedSched::Bpfland, SchedMode::Gaming),
            vec!["-m", "performance"]
        );
    }

    #[test]
    fn parse_and_serialize_config() {
        let config = default_config();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed = parse_config(&toml_str).unwrap();
        assert_eq!(parsed, config);
        assert!(parse_config("").is_err());
        // serde shape: default_sched = "scx_bpfland", default_mode = "Auto"
        assert!(toml_str.contains("default_mode = \"Auto\""));
    }
}
