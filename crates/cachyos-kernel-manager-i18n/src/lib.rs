//! Locale resolution and translation-catalog inventory.
//!
//! Reconstructed from `main.cpp:62-106` (`initTranslations`, bitcoin-qt
//! derived) and `cachyoskm_locale.qrc`.
//!
//! Qt semantics reproduced:
//! - desired locale = `QLocale::system().name()` (e.g. `de_DE`, `C`),
//! - base language = truncated at the last `_` (e.g. `de_DE` → `de`);
//!   `QString::truncate(pos)` with no `_` (pos = -1) leaves the string
//!   unchanged (courted: `i18n/C-locale`),
//! - load order: `qt_<base>`, `qt_<lang_territory>`, app `<base>`, app
//!   `<lang_territory>`,
//! - app catalogs live under the `:/translations/` resource with aliases
//!   equal to the bare language code.
//!
//! Known quirk (courted): `cachyos-kernel-manager_uk.ts` exists in `lang/`
//! but is absent from `cachyoskm_locale.qrc` — Ukrainian is never loaded by
//! the oracle. The candidate reproduces the *loaded* catalog set.

#![forbid(unsafe_code)]

/// Locales compiled into the resource (`cachyoskm_locale.qrc` aliases, in
/// file order).
pub const COMPILED_LOCALES: &[&str] = &[
    "bg", "ca", "cs", "de", "es", "it", "ja", "ko", "pl", "ru", "sk", "sv", "tr", "nl", "zh-CN",
];

/// Translation source files present in `lang/` but NOT compiled into the
/// resource (quirk).
pub const UNCOMPILED_TS_LOCALES: &[&str] = &["uk"];

/// One catalog to attempt loading.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CatalogAttempt {
    /// The catalog key (e.g. `de`, `de_DE`, `en`).
    pub key: String,
    /// Whether it is a Qt-provided catalog (`qt_<key>.qm`) or an app catalog
    /// (app `<key>.qm` under `:/translations/`).
    pub qt: bool,
}

/// Derive the base language from a `lang_TERRITORY` string the way Qt's
/// `initTranslations` does (`truncate` at the last `_`; no change when there
/// is no `_`).
pub fn base_language(lang_territory: &str) -> &str {
    match lang_territory.rfind('_') {
        Some(pos) => &lang_territory[..pos],
        None => lang_territory,
    }
}

/// The full load order for a system locale string, matching
/// `initTranslations`.
pub fn load_order(system_locale: &str) -> Vec<CatalogAttempt> {
    let base = base_language(system_locale);
    vec![
        CatalogAttempt {
            key: base.to_string(),
            qt: true,
        },
        CatalogAttempt {
            key: system_locale.to_string(),
            qt: true,
        },
        CatalogAttempt {
            key: base.to_string(),
            qt: false,
        },
        CatalogAttempt {
            key: system_locale.to_string(),
            qt: false,
        },
    ]
}

/// Which app catalogs we can actually load (compiled set).
pub fn available_app_locales() -> &'static [&'static str] {
    COMPILED_LOCALES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_language_truncates_at_last_underscore() {
        assert_eq!(base_language("de_DE"), "de");
        assert_eq!(base_language("zh_CN"), "zh");
        assert_eq!(base_language("en_US"), "en");
        // no underscore -> unchanged (Qt truncate(-1) semantics, courted)
        assert_eq!(base_language("C"), "C");
        assert_eq!(base_language("de"), "de");
    }

    #[test]
    fn load_order_matches_qt() {
        let order = load_order("de_DE");
        let keys: Vec<(&str, bool)> = order.iter().map(|c| (c.key.as_str(), c.qt)).collect();
        assert_eq!(
            keys,
            vec![
                ("de", true),
                ("de_DE", true),
                ("de", false),
                ("de_DE", false)
            ]
        );
    }

    #[test]
    fn uk_translation_is_not_compiled() {
        assert!(!COMPILED_LOCALES.contains(&"uk"));
        assert!(UNCOMPILED_TS_LOCALES.contains(&"uk"));
    }
}
