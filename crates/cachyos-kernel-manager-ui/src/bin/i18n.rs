//! `cachyos-kernel-manager-i18n` — candidate witness for the
//! `ui/i18n-resolution` court. Reads the corpus (system locale + lookups)
//! and renders the candidate's REAL i18n layer
//! (`crates/cachyos-kernel-manager-ui/src/i18n.rs`: the embedded ts2json
//! catalogs + the `initTranslations` resolution): the resolved qrc alias
//! and the translated texts.
//!
//! Usage: cachyos-kernel-manager-i18n parse <corpus.json>

use cachyos_kernel_manager_ui::i18n::resolve;
use serde::Deserialize;
use serde_json::json;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Lookup {
    context: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct Corpus {
    system_locale: String,
    lookups: Vec<Lookup>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("usage: cachyos-kernel-manager-i18n parse <corpus.json>");
        return ExitCode::from(2);
    };
    if cmd != "parse" {
        eprintln!("usage: cachyos-kernel-manager-i18n parse <corpus.json>");
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
    let locale = resolve(&corpus.system_locale);
    let mut lookups = Vec::new();
    for lookup in &corpus.lookups {
        lookups.push(json!({
            "context": lookup.context,
            "source": lookup.source,
            "translation": locale.tr(&lookup.context, &lookup.source),
        }));
    }
    let payload = json!({
        "schema": "cachyos-km-i18n-v1",
        "system_locale": corpus.system_locale,
        "resolved": locale.catalog,
        "lookups": lookups,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
