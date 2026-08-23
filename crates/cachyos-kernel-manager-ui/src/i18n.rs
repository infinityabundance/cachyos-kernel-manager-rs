//! The Slint UI's translation layer (Phase 8) — the candidate side of the
//! `ui/i18n-resolution` court.
//!
//! Reconstructed from `main.cpp:62-106` (`initTranslations`) +
//! `cachyoskm_locale.qrc`:
//!
//! - the desired locale is `QLocale::system().name()` (e.g. `de_DE`, `zh_CN`);
//! - the load order is `qt_<base>`, `qt_<lang_territory>`, app `<base>`,
//!   app `<lang_territory>` ([`cachyos_kernel_manager_i18n::load_order`],
//!   courted by `i18n/C-locale`);
//! - app catalogs are the qrc aliases (the BARE code, e.g. `de`, `zh-CN`);
//!   a catalog loads iff its alias is among the compiled set
//!   ([`available_app_locales`]).
//!
//! The translation DATA is derived from the frozen `.ts` files
//! (`oracle/upstream/lang/`, revision `6b4a373e`) by `tools/ts2json.py`
//! into the embedded catalogs here; the derivation + resolution are courted
//! by `ui/i18n-resolution` (the oracle-ref re-parses the `.ts` directly).
//!
//! Qt `QTranslator::translate(context, source)` semantics reproduced:
//! an entry whose translation is empty or `type="unfinished"`/`vanished` is
//! skipped (the source text is returned); the FIRST matching
//! (context, source) pair wins.
//!
//! gap-009: the qrc alias for Chinese is `zh-CN`, but a `zh_CN` system
//! locale produces load-order keys `zh` and `zh_CN` — neither is a compiled
//! alias — so the oracle's `zh-CN` catalog is NEVER reached for a plain
//! `zh_CN` locale. This module reproduces exactly that (English fallback);
//! the question of whether the oracle loads it through some other path stays
//! open (gap-009).

use cachyos_kernel_manager_i18n::{available_app_locales, load_order};
use serde::Deserialize;
use std::sync::OnceLock;

/// One `<message>` of a frozen `.ts` file (ts2json output).
#[derive(Debug, Clone, Deserialize)]
pub struct TranslationEntry {
    pub context: String,
    pub source: String,
    pub translation: String,
    /// `<translation type="unfinished|vanished">` — not translated by Qt.
    pub unfinished: bool,
    #[serde(default)]
    pub locations: Vec<String>,
}

/// One embedded locale catalog (ts2json output for one `.ts`).
#[derive(Debug, Clone, Deserialize)]
pub struct LocaleCatalog {
    pub locale: String,
    /// The qrc alias (the KEY the resolver matches).
    pub qrc_alias: String,
    pub source_ts: String,
    pub source_sha256: String,
    pub entries: Vec<TranslationEntry>,
}

/// The resolved locale: which catalog (if any) is active for a system
/// locale, per `initTranslations` + the qrc alias set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedLocale {
    /// The active qrc alias (e.g. `de`, `zh-CN`); `None` = English (no
    /// catalog matched — the oracle falls back to the source text).
    pub catalog: Option<&'static str>,
}

impl ResolvedLocale {
    /// The catalog, if any.
    pub fn catalog(&self) -> Option<&'static LocaleCatalog> {
        self.catalog.and_then(catalog_by_alias)
    }

    /// Translate a source text in its Qt context.
    ///
    /// Falls back to the source text when no catalog is active, the
    /// (context, source) pair is absent, or the translation is
    /// empty/unfinished — exactly `QTranslator::translate`.
    pub fn tr<'a>(&'a self, context: &str, source: &'a str) -> &'a str {
        match self.catalog() {
            Some(cat) => cat.translate(context, source),
            None => source,
        }
    }
}

/// The resolved locale for a system locale string (the app-level part of
/// `initTranslations`; Qt-provided catalogs are skipped — the candidate has
/// no Qt runtime).
pub fn resolve(system_locale: &str) -> ResolvedLocale {
    let attempts = load_order(system_locale);
    for attempt in &attempts {
        if attempt.qt {
            continue; // qt_<key>.qm catalogs are Qt's; not app resources
        }
        if let Some(alias) = app_alias(&attempt.key) {
            return ResolvedLocale {
                catalog: Some(alias),
            };
        }
    }
    ResolvedLocale { catalog: None }
}

/// Map a load-order key to a compiled qrc alias, if any. The alias set is
/// `available_app_locales()` (the bare codes); a key matches only exactly
/// (`zh_CN` ≠ `zh-CN` — gap-009).
fn app_alias(key: &str) -> Option<&'static str> {
    available_app_locales().iter().copied().find(|a| *a == key)
}

/// All embedded catalogs, parsed once.
fn catalogs() -> &'static [LocaleCatalog] {
    static CATALOGS: OnceLock<Vec<LocaleCatalog>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        available_app_locales()
            .iter()
            .filter_map(|alias| catalog_json(alias))
            .collect()
    })
}

/// Find a catalog by its qrc alias.
pub fn catalog_by_alias(alias: &str) -> Option<&'static LocaleCatalog> {
    catalogs().iter().find(|c| c.qrc_alias == alias)
}

/// Parse one embedded catalog JSON (compile-time `include_str!`).
fn catalog_json(alias: &str) -> Option<LocaleCatalog> {
    let raw = match alias {
        "bg" => include_str!("../translations/bg.json"),
        "ca" => include_str!("../translations/ca.json"),
        "cs" => include_str!("../translations/cs.json"),
        "de" => include_str!("../translations/de.json"),
        "es" => include_str!("../translations/es.json"),
        "it" => include_str!("../translations/it.json"),
        "ja" => include_str!("../translations/ja.json"),
        "ko" => include_str!("../translations/ko.json"),
        "nl" => include_str!("../translations/nl.json"),
        "pl" => include_str!("../translations/pl.json"),
        "ru" => include_str!("../translations/ru.json"),
        "sk" => include_str!("../translations/sk.json"),
        "sv" => include_str!("../translations/sv.json"),
        "tr" => include_str!("../translations/tr.json"),
        "zh-CN" => include_str!("../translations/zh-CN.json"),
        _ => return None,
    };
    serde_json::from_str(raw).ok()
}

impl LocaleCatalog {
    /// `QTranslator::translate(context, source)`: first matching
    /// (context, source) with a non-empty, finished translation; else the
    /// source text.
    pub fn translate<'a>(&'a self, context: &str, source: &'a str) -> &'a str {
        self.entries
            .iter()
            .find(|e| e.context == context && e.source == source)
            .filter(|e| !e.unfinished && !e.translation.is_empty())
            .map(|e| e.translation.as_str())
            .unwrap_or(source)
    }

    /// The locale's display name (the qrc alias).
    pub fn alias(&self) -> &str {
        &self.qrc_alias
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_matches_qrc_alias_set() {
        // de_DE -> qt de, qt de_DE, app de (compiled) -> de
        let r = resolve("de_DE");
        assert_eq!(r.catalog, Some("de"));
        // bare de -> qt de, app de
        let r = resolve("de");
        assert_eq!(r.catalog, Some("de"));
        // zh_CN -> zh and zh_CN are NOT compiled aliases (gap-009)
        let r = resolve("zh_CN");
        assert_eq!(r.catalog, None);
        // a compiled dash-alias locale reaches its catalog exactly
        let r = resolve("zh-CN");
        assert_eq!(r.catalog, Some("zh-CN"));
        // unknown locale -> English
        let r = resolve("xx_YY");
        assert_eq!(r.catalog, None);
        // C locale -> no catalog (courted by i18n/C-locale semantics)
        let r = resolve("C");
        assert_eq!(r.catalog, None);
    }

    #[test]
    fn zh_cn_catalog_has_the_expected_surface() {
        let cat = catalog_by_alias("zh-CN").expect("zh-CN compiled");
        assert_eq!(cat.locale, "zh-CN");
        assert!(!cat.entries.is_empty());
        // a known translation pair from the frozen .ts
        let t = cat.translate("ConfOptionsPage", "Enable CachyOS config");
        assert_eq!(t, "启用 CachyOS 优化的内核配置");
        // absent source -> the source text (Qt fallback)
        let t = cat.translate("MainWindow", "definitely-not-a-string");
        assert_eq!(t, "definitely-not-a-string");
    }

    #[test]
    fn resolution_falls_back_to_english_without_catalog() {
        let r = resolve("en_US");
        assert_eq!(r.catalog, None);
        assert_eq!(r.tr("MainWindow", "Execute"), "Execute");
    }

    #[test]
    fn catalogs_are_all_compiled_and_parseable() {
        for alias in available_app_locales() {
            let cat = catalog_by_alias(alias).unwrap_or_else(|| panic!("{alias} missing"));
            assert_eq!(cat.qrc_alias, *alias);
        }
    }
}
