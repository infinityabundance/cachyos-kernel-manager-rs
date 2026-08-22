//! Faithful port of the oracle's pacman.conf parser.
//!
//! The oracle does NOT use pacman's config parser — it uses the vendored
//! `mINI` parser (`oracle/upstream/src/ini.hpp`, revision `6b4a373e`) and
//! registers every section except `testing` and `options` as a sync database
//! (`src/alpm_utils.cpp:32-47`).
//!
//! mINI semantics reproduced here exactly:
//! - section names and keys are **lowercased** (`ini.hpp:126` `toLower`,
//!   `MINI_CASE_SENSITIVE` is not defined),
//! - order of first appearance is preserved (vector + index map),
//! - duplicated sections merge into the first position (real pacman errors
//!   on duplicated repositories — a captured, courted difference),
//! - `#` is a comment only at line start; `;` comments are stripped from
//!   section lines; `;`-starting lines are comments,
//! - `key=value` splits on the first `=`, honoring `\=` escaping,
//! - `\r` and NUL bytes are dropped,
//! - `Include` directives are treated as ordinary keys and NOT followed (a
//!   repo defined only inside an included file is invisible to the oracle),
//! - keys before any section land in auto-named numeric sections
//!   (`"0"`, `"1"`, …) which the oracle registers as repositories (!) —
//!   preserved for parity.
//!
//! Courts: `pacman-config/*`.

#![forbid(unsafe_code)]

use std::collections::HashMap;

/// A parsed INI document: ordered, lowercased section names mapping to
/// ordered, lowercased key/value pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniIni {
    sections: Vec<(String, Vec<(String, String)>)>,
    index: HashMap<String, usize>,
}

impl MiniIni {
    /// Section names in order of first appearance (already lowercased).
    pub fn section_names(&self) -> Vec<&str> {
        self.sections
            .iter()
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Keys of a section (already lowercased), in order.
    pub fn keys_of(&self, section: &str) -> Vec<&str> {
        self.sections
            .iter()
            .find(|(name, _)| name == section)
            .map(|(_, kv)| kv.iter().map(|(k, _)| k.as_str()).collect())
            .unwrap_or_default()
    }

    /// Parse INI content with mINI's exact line semantics.
    pub fn parse(content: &str) -> MiniIni {
        let mut ini = MiniIni {
            sections: Vec::new(),
            index: HashMap::new(),
        };
        let mut section: String = String::new();
        let mut in_section = false;
        // mINI numbers pre-section key/value blocks independently of the
        // section count (`std::to_string(repeated)`, ini.hpp:326)
        let mut pre_section_counter: usize = 0;

        for raw_line in content.split('\n') {
            // readFile(): drop NUL and CR
            let line: String = raw_line
                .chars()
                .filter(|c| *c != '\0' && *c != '\r')
                .collect();
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let first = line.chars().next().expect("non-empty");
            if first == ';' {
                continue;
            }
            if first == '[' {
                let no_comment = match line.find(';') {
                    Some(pos) => &line[..pos],
                    None => line,
                };
                if let Some(closing) = no_comment.rfind(']') {
                    section = no_comment[1..closing].trim().to_ascii_lowercase();
                    ini.ensure_section(&section);
                    in_section = true;
                    continue;
                }
                // mINI: a '[' line WITHOUT a closing ']' does NOT return from
                // parseLine — it FALLS THROUGH to the key/value branch
                // (ini.hpp parseLine: the `if (closingBracketAt != npos)`
                // block is skipped, the '=' scan still runs). So `[a=b`
                // becomes a key `[a` in the current (or auto) section.
            }
            // key=value with \= escaping
            let line_norm = line.replace("\\=", "  ");
            if let Some(equals_at) = line_norm.find('=') {
                let key = line[..equals_at]
                    .trim()
                    .replace("\\=", "=")
                    .to_ascii_lowercase();
                let value = line[equals_at + 1..].trim().to_string();
                if in_section {
                    ini.set_value(&section, &key, value);
                } else {
                    // keys before any section land in numeric auto-sections
                    let auto = format!("{pre_section_counter}");
                    pre_section_counter += 1;
                    ini.ensure_section(&auto);
                    ini.set_value(&auto, &key, value);
                }
            }
            // PDATA_UNKNOWN lines are simply ignored
        }
        ini
    }

    fn ensure_section(&mut self, name: &str) {
        if !self.index.contains_key(name) {
            self.index.insert(name.to_string(), self.sections.len());
            self.sections.push((name.to_string(), Vec::new()));
        }
    }

    fn set_value(&mut self, section: &str, key: &str, value: String) {
        let idx = *self.index.get(section).expect("section ensured");
        let kv = &mut self.sections[idx].1;
        match kv.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => kv.push((key.to_string(), value)),
        }
    }
}

/// The oracle's registration rule (`alpm_utils.cpp:41-47`): skip `testing`
/// and `options`; register every other section, in order.
pub fn register_sections(ini: &MiniIni) -> Vec<String> {
    ini.section_names()
        .into_iter()
        .filter(|s| *s != "testing" && *s != "options")
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDINARY: &str = r#"
# global options
[options]
ParallelDownloads = 5

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[cachyos]
Server = https://mirror.cachyos.org/repo/x86_64/cachyos
"#;

    #[test]
    fn sections_in_order_and_lowercased() {
        let ini = MiniIni::parse(ORDINARY);
        assert_eq!(
            ini.section_names(),
            vec!["options", "core", "extra", "cachyos"]
        );
        assert_eq!(ini.keys_of("cachyos"), vec!["server"]);
        // keys lowercased, values trimmed
        assert_eq!(register_sections(&ini), vec!["core", "extra", "cachyos"]);
    }

    #[test]
    fn testing_and_options_skipped_even_when_mixed_case() {
        // mINI lowercases stored section names -> [Testing] IS skipped
        let ini = MiniIni::parse("[options]\n[Testing]\n[core]\n");
        assert_eq!(register_sections(&ini), vec!["core"]);
    }

    #[test]
    fn duplicated_sections_merge_into_first_position() {
        // real pacman errors here; mINI merges silently (captured difference)
        let ini = MiniIni::parse("[core]\nServer = a\n[core]\nServer = b\n[extra]\n");
        assert_eq!(ini.section_names(), vec!["core", "extra"]);
        // value overwritten by the later duplicate
        assert_eq!(ini.keys_of("core"), vec!["server"]);
    }

    #[test]
    fn include_is_an_ordinary_key_not_followed() {
        let ini = MiniIni::parse("[repo]\nInclude = /etc/pacman.d/other\n");
        assert_eq!(ini.keys_of("repo"), vec!["include"]);
        // the repo is still registered (Include not followed)
        assert_eq!(register_sections(&ini), vec!["repo"]);
    }

    #[test]
    fn comment_and_escaping_semantics() {
        let ini = MiniIni::parse(
            "# comment\n; comment\n[a]\nkey = value # trailing is NOT a comment\nb\\=x = y\n",
        );
        assert_eq!(ini.section_names(), vec!["a"]);
        assert_eq!(ini.keys_of("a"), vec!["key", "b=x"]);
    }

    #[test]
    fn keys_before_any_section_become_numeric_sections() {
        // mINI: key=value lines before any section land in auto-named
        // sections "0", "1", ... which the oracle registers as repositories
        // (parity, courted). Lines without '=' are PDATA_UNKNOWN and ignored.
        let ini = MiniIni::parse("Color = 1\nColor2 = 2\n[core]\n");
        assert_eq!(ini.section_names(), vec!["0", "1", "core"]);
        assert_eq!(register_sections(&ini), vec!["0", "1", "core"]);
        // a line without '=' is ignored entirely
        let ini = MiniIni::parse("Color\n[core]\n");
        assert_eq!(ini.section_names(), vec!["core"]);
    }

    #[test]
    fn cr_and_nul_dropped() {
        let ini = MiniIni::parse("[a]\r\nx = 1\r\n[b]\u{0}\n");
        assert_eq!(ini.section_names(), vec!["a", "b"]);
    }

    #[test]
    fn section_line_without_closing_bracket_falls_through_to_keyvalue() {
        // mINI parseLine: `[a=b` has no ']' so the section block is skipped
        // and the '=' scan runs — the line becomes a key `[a` (value `b`).
        // Before any real section this lands in the numeric auto-section.
        let ini = MiniIni::parse("[a=b\n[core]\n");
        assert_eq!(ini.section_names(), vec!["0", "core"]);
        assert_eq!(ini.keys_of("0"), vec!["[a"]);
        assert_eq!(register_sections(&ini), vec!["0", "core"]);

        // inside a section the key lands there
        let ini = MiniIni::parse("[sec]\n[a=b\n");
        assert_eq!(ini.keys_of("sec"), vec!["[a"]);

        // `[broken` (no '=', no ']') is PDATA_UNKNOWN -> ignored entirely
        let ini = MiniIni::parse("[broken\n[core]\n");
        assert_eq!(ini.section_names(), vec!["core"]);
    }
}
