//! TOML configuration schema, byte-compatible with the oracle's
//! `config-option-lib` (`config-option-lib/src/lib.rs`, revision
//! `6b4a373e`).
//!
//! Compatibility surfaces preserved:
//! - field names and **serialization order** (struct declaration order;
//!   `toml::to_string` writes fields in declaration order),
//! - `#[serde(default)]` on every field: omitted fields default to
//!   false/empty (the oracle's `Config` derives `Default` + `serde(default)`),
//! - unknown fields are ignored (no `deny_unknown_fields` in the oracle),
//! - string fields are free-form (invalid enum-like values are tolerated on
//!   load and flagged "outdated" by the UI layer).
//!
//! Divergence (documented, D-002): the oracle's `write_config_file` uses
//! `File::create` (truncate-in-place, no fsync). The candidate writes a
//! temporary sibling and renames atomically, retaining before/after
//! evidence. Court: `config-roundtrip/*`.
//!
//! Note: `per_gov_check` in the schema maps to the UI widget
//! `perfgovern_check` and the build var `_per_gov` — the naming mismatch is
//! the oracle's own.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Error type for config load/save.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// TOML parse failure (surfaced by the UI as "Failed to load config
    /// options from file: ...").
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    /// TOML serialization failure.
    #[error("toml ser error: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

/// The 18-field configuration, in the oracle's exact declaration order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KernelManagerConfig {
    pub hardly_check: bool,
    pub per_gov_check: bool,
    pub tcp_bbr3_check: bool,
    pub cachy_config_check: bool,
    pub nconfig_check: bool,
    pub xconfig_check: bool,
    pub localmodcfg_check: bool,
    pub use_current_check: bool,
    pub builtin_zfs_check: bool,
    pub builtin_nvidia_open_check: bool,
    pub build_debug_check: bool,
    pub hz_ticks_combo: String,
    pub tickrate_combo: String,
    pub preempt_combo: String,
    pub hugepage_combo: String,
    pub lto_combo: String,
    pub cpu_opt_combo: String,
    pub custom_name_edit: String,
}

impl KernelManagerConfig {
    /// Parse from string content (`parse_config` in the oracle).
    pub fn parse(content: &str) -> Result<KernelManagerConfig, ConfigError> {
        Ok(toml::from_str(content)?)
    }

    /// Serialize to TOML (`toml::to_string` in the oracle).
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string(self)?)
    }

    /// Load from a file.
    pub fn load(path: &Path) -> Result<KernelManagerConfig, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Save atomically (D-002): temp sibling + rename; the oracle's
    /// non-atomic write is intentionally not reproduced.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = self.to_toml_string()?;
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = dir.join(format!(
            ".{}.tmp.{}",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("config"),
            std::process::id()
        ));
        // ensure uniqueness even within the same process
        let mut n = 0u32;
        while tmp.exists() {
            n += 1;
            tmp = dir.join(format!(
                ".{}.tmp.{}.{n}",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("config"),
                std::process::id()
            ));
        }
        std::fs::write(&tmp, content)?;
        match std::fs::rename(&tmp, path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(ConfigError::Io(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_matches_oracle_field_order() {
        let cfg = KernelManagerConfig {
            hardly_check: true,
            per_gov_check: false,
            tcp_bbr3_check: false,
            cachy_config_check: true,
            nconfig_check: false,
            xconfig_check: false,
            localmodcfg_check: false,
            use_current_check: false,
            builtin_zfs_check: true,
            builtin_nvidia_open_check: false,
            build_debug_check: false,
            hz_ticks_combo: "1000".into(),
            tickrate_combo: "full".into(),
            preempt_combo: "full".into(),
            hugepage_combo: "always".into(),
            lto_combo: "thin".into(),
            cpu_opt_combo: "manual".into(),
            custom_name_edit: "$pkgbase-custom".into(),
        };
        let s = cfg.to_toml_string().unwrap();
        // toml::to_string emits all fields in declaration order; spot-check
        // the head of the file and the ordering of string fields.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[0], "hardly_check = true");
        assert_eq!(lines[1], "per_gov_check = false");
        assert!(lines.contains(&"hz_ticks_combo = \"1000\""));
        assert!(lines.contains(&"custom_name_edit = \"$pkgbase-custom\""));
        let pos = |field: &str| lines.iter().position(|l| l.starts_with(field)).unwrap();
        assert!(pos("hz_ticks_combo") < pos("tickrate_combo"));
        assert!(pos("tickrate_combo") < pos("preempt_combo"));
        assert!(pos("lto_combo") < pos("cpu_opt_combo"));
        assert!(pos("cpu_opt_combo") < pos("custom_name_edit"));
    }

    #[test]
    fn omitted_fields_default() {
        let cfg = KernelManagerConfig::parse("hardly_check = true\n").unwrap();
        assert!(cfg.hardly_check);
        assert!(!cfg.cachy_config_check);
        assert_eq!(cfg.hz_ticks_combo, "");
    }

    #[test]
    fn unknown_fields_ignored_like_oracle() {
        let cfg =
            KernelManagerConfig::parse("hardly_check = true\nfuture_field = \"x\"\n").unwrap();
        assert!(cfg.hardly_check);
    }

    #[test]
    fn round_trip_preserves_semantics() {
        let cfg = KernelManagerConfig {
            hardly_check: true,
            per_gov_check: true,
            tcp_bbr3_check: false,
            cachy_config_check: false,
            nconfig_check: true,
            xconfig_check: false,
            localmodcfg_check: false,
            use_current_check: true,
            builtin_zfs_check: false,
            builtin_nvidia_open_check: true,
            build_debug_check: false,
            hz_ticks_combo: "300".into(),
            tickrate_combo: "idle".into(),
            preempt_combo: "lazy".into(),
            hugepage_combo: "madvise".into(),
            lto_combo: "none".into(),
            cpu_opt_combo: "zen4".into(),
            custom_name_edit: "my-kernel".into(),
        };
        let s = cfg.to_toml_string().unwrap();
        let back = KernelManagerConfig::parse(&s).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(KernelManagerConfig::parse("not = [valid").is_err());
    }

    #[test]
    fn save_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("km-rs-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.toml");
        let cfg = KernelManagerConfig {
            cachy_config_check: true,
            hz_ticks_combo: "750".into(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = KernelManagerConfig::load(&path).unwrap();
        assert_eq!(cfg, loaded);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
