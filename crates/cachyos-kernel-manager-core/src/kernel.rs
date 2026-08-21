//! Kernel identity, category classification, and version display semantics.
//!
//! Reconstructed from `oracle/upstream/src/kernel.hpp` and
//! `oracle/upstream/src/kernel.cpp` (revision `6b4a373e`).
//! Protected by courts: `kernel-discovery/*`, `version-state/*`, `category/*`.

use std::cmp::Ordering;

/// Version marker for *downgrade* (local newer than sync): `∨` (U+2228).
/// Source: `kernel.cpp:72` — `fmt::format(FMT_COMPILE("∨{}"), local_pkg_ver)`.
pub const DOWNGRADE_MARKER: char = '\u{2228}';

/// Version marker for *update available* (sync newer than local): `∧` (U+2227).
/// Source: `kernel.cpp:75` — `fmt::format(FMT_COMPILE("∧{}"), sync_pkg_ver)`.
pub const UPDATE_MARKER: char = '\u{2227}';

/// Version string shown for AUR kernels.
/// Source: `kernel.cpp:277` — `m_version = "unknown-version"`.
pub const AUR_VERSION: &str = "unknown-version";

/// A kernel package name (no repo prefix). e.g. `linux-cachyos`.
///
/// Newtype keeps package-name validation in one place; the ALPM layer is the
/// authority for validity, this type only carries the invariant "no '/'".
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct KernelName(String);

impl KernelName {
    /// Creates a kernel name. Returns `None` if the name contains a `/`
    /// (repo-qualified names are the separate `raw` display form, never a
    /// package identity) or is empty.
    pub fn new(name: impl Into<String>) -> Option<Self> {
        let name = name.into();
        if name.is_empty() || name.contains('/') {
            return None;
        }
        Some(Self(name))
    }

    /// The raw package name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KernelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The category column value, reconstructed from `Kernel::category()`
/// (`kernel.hpp:37-92`). The oracle scans substrings in a fixed order and
/// returns the first hit; the display strings are byte-for-byte the oracle's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KernelCategory {
    /// `lto` -> "lto optimized"
    LtoOptimized,
    /// `lts` -> "longterm"
    Longterm,
    /// `zen` -> "zen-kernel"
    Zen,
    /// `hardened` -> "hardened kernel"
    Hardened,
    /// `deckify` -> "handheld kernel"
    Deckify,
    /// `server` -> "server kernel"
    Server,
    /// `next` -> "next release"
    NextRelease,
    /// `mainline` -> "mainline branch"
    Mainline,
    /// `git` -> "master branch"
    Git,
    /// `rc` -> "release candidate"
    Rc,
    /// fallback -> "stable"
    Stable,
}

impl KernelCategory {
    /// The exact display string used by the oracle (also the tree column text).
    pub fn display(self) -> &'static str {
        match self {
            KernelCategory::LtoOptimized => "lto optimized",
            KernelCategory::Longterm => "longterm",
            KernelCategory::Zen => "zen-kernel",
            KernelCategory::Hardened => "hardened kernel",
            KernelCategory::Deckify => "handheld kernel",
            KernelCategory::Server => "server kernel",
            KernelCategory::NextRelease => "next release",
            KernelCategory::Mainline => "mainline branch",
            KernelCategory::Git => "master branch",
            KernelCategory::Rc => "release candidate",
            KernelCategory::Stable => "stable",
        }
    }
}

/// Classify a kernel package name exactly like the oracle's `category()`:
/// first substring match wins, scanning in this order:
/// `lto, lts, zen, hardened, deckify, server, next, mainline, git, rc`.
///
/// This is a *substring* search (`std::ranges::search`), not a prefix search:
/// e.g. `linux-cachyos-lts` contains `lts` -> Longterm; a hypothetical
/// `linux-zen-rt` would be Zen (zen is scanned before rt is even considered;
/// `rt` is not in the list at all).
pub fn classify_category(name: &str) -> KernelCategory {
    const NEEDLES: &[(&str, KernelCategory)] = &[
        ("lto", KernelCategory::LtoOptimized),
        ("lts", KernelCategory::Longterm),
        ("zen", KernelCategory::Zen),
        ("hardened", KernelCategory::Hardened),
        ("deckify", KernelCategory::Deckify),
        ("server", KernelCategory::Server),
        ("next", KernelCategory::NextRelease),
        ("mainline", KernelCategory::Mainline),
        ("git", KernelCategory::Git),
        ("rc", KernelCategory::Rc),
    ];
    for (needle, category) in NEEDLES {
        if name.contains(needle) {
            return *category;
        }
    }
    KernelCategory::Stable
}

/// Whether a package name matches the oracle's libalpm search needle
/// `linux[^ ]*-headers` (`kernel.cpp:187`).
///
/// The needle is a POSIX regex matched against package names by
/// `alpm_db_search`. Because package names cannot contain spaces, the regex
/// is equivalent to: some substring `linux` followed (possibly immediately)
/// by non-space characters, then by the literal `-headers`.
///
/// Implemented directly (not via a regex engine) so the behavior is
/// deterministic and testable; note the regex is *unanchored*, so
/// `linux-foo-headers-bar` DOES match (the substring `linux-foo-headers`
/// matches), even though the name does not end with `-headers`. This is a
/// deliberate, courted fidelity detail (`kernel-discovery/needle` court).
pub fn matches_headers_needle(name: &str) -> bool {
    let bytes = name.as_bytes();
    let linux = b"linux";
    let headers = b"-headers";
    let mut i = 0;
    while i + linux.len() <= bytes.len() {
        if bytes[i..].starts_with(linux) {
            // find "-headers" at or after i + linux.len()
            let tail = &bytes[i + linux.len()..];
            if let Some(rel) = find_subslice(tail, headers) {
                let between = &tail[..rel];
                if between.iter().all(|b| *b != b' ') {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Convert a headers package name to its kernel name exactly like the oracle:
/// `utils::remove_all(pkg_name, "-headers")` removes **every** occurrence of
/// `-headers`, not only a suffix (`kernel.cpp:207`).
pub fn kernel_headers_name(headers_name: &str) -> String {
    headers_name.replace("-headers", "")
}

/// Whether a package name must be excluded from discovery: contains the
/// substring `linux-api-headers` (`kernel.cpp:180,201-204`).
pub fn is_api_headers(name: &str) -> bool {
    name.contains("linux-api-headers")
}

/// Version display semantics from `Kernel::version()` (`kernel.cpp:56-79`).
///
/// Modeled as a pure function over the ALPM version comparison outcome so it
/// can be courted without libalpm; the real comparator comes from the ALPM
/// layer (`alpm_pkg_vercmp`), never semver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpdateFlag {
    /// Installed version is newer than sync (downgrade); marker `∨`.
    Downgrade,
    /// Sync version is newer (update available); marker `∧`; sets the
    /// oracle's `m_update` flag so re-installation is allowed.
    UpdateAvailable,
}

/// The rendered version string plus the update flag.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayVersion {
    /// Text shown in the Version column.
    pub text: String,
    /// Whether the kernel is flagged as needing an update.
    pub update: bool,
}

impl DisplayVersion {
    /// Reconstructs `Kernel::version()`:
    ///
    /// - AUR kernels: `unknown-version` (no update flag).
    /// - Not installed: sync version.
    /// - Installed: `vercmp(local, sync)` —
    ///   1 -> `∨<local>`, -1 -> `∧<sync>` (+ update flag), 0 -> sync version.
    pub fn for_aur() -> Self {
        DisplayVersion {
            text: AUR_VERSION.into(),
            update: false,
        }
    }

    /// Reconstructs `Kernel::version()`:
    ///
    /// - AUR kernels: `unknown-version` (no update flag).
    /// - Not installed: sync version.
    /// - Installed: `vercmp(local, sync)` —
    ///   1 -> `∨<local>`, -1 -> `∧<sync>` (+ update flag), 0 -> sync version.
    pub fn compute(
        installed: Option<&str>,
        sync: &str,
        vercmp: impl FnOnce(&str, &str) -> Ordering,
    ) -> Self {
        match installed {
            None => DisplayVersion {
                text: sync.to_owned(),
                update: false,
            },
            Some(local) => match vercmp(local, sync) {
                Ordering::Greater => DisplayVersion {
                    text: format!("{DOWNGRADE_MARKER}{local}"),
                    update: false,
                },
                Ordering::Less => DisplayVersion {
                    text: format!("{UPDATE_MARKER}{sync}"),
                    update: true,
                },
                Ordering::Equal => DisplayVersion {
                    text: sync.to_owned(),
                    update: false,
                },
            },
        }
    }
}

/// Strip a `∨`/`∧` marker prefix, mirroring `KernelTreeWidgetItem::operator<`
/// (`km-window.cpp:397-406`) which sorts the Version column by the
/// marker-less value using `alpm_pkg_vercmp`.
pub fn strip_version_marker(text: &str) -> &str {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c == DOWNGRADE_MARKER || c == UPDATE_MARKER => chars.as_str(),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vcmp(a: &str, b: &str) -> Ordering {
        // Test comparator; real ALPM vercmp is supplied by the alpm layer.
        // Segment-wise numeric comparison is enough for these semantics
        // tests (e.g. 6.13.9-1 vs 6.14.1-1).
        let seg = |s: &str| -> Vec<String> {
            s.split(|c: char| !c.is_ascii_digit() && c != '.')
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect()
        };
        let (sa, sb) = (seg(a), seg(b));
        for (x, y) in sa.iter().zip(sb.iter()) {
            match (x.parse::<u64>().ok(), y.parse::<u64>().ok()) {
                (Some(xn), Some(yn)) if xn != yn => return xn.cmp(&yn),
                _ => {
                    let ord = x.cmp(y);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
        }
        sa.len().cmp(&sb.len())
    }

    #[test]
    fn category_order_and_substring_semantics() {
        assert_eq!(classify_category("linux-cachyos").display(), "stable");
        assert_eq!(classify_category("linux-cachyos-lts").display(), "longterm");
        assert_eq!(classify_category("linux-lts").display(), "longterm");
        assert_eq!(
            classify_category("linux-cachyos-hardened").display(),
            "hardened kernel"
        );
        assert_eq!(
            classify_category("linux-cachyos-deckify").display(),
            "handheld kernel"
        );
        assert_eq!(
            classify_category("linux-cachyos-server").display(),
            "server kernel"
        );
        assert_eq!(
            classify_category("linux-cachyos-rc").display(),
            "release candidate"
        );
        // substring, not prefix: the needle can appear mid-name
        assert_eq!(classify_category("foo-lts-bar").display(), "longterm");
        // precedence: lto is scanned before lts
        assert_eq!(
            classify_category("linux-cachyos-lto-lts").display(),
            "lto optimized"
        );
        // 'git' substring matches inside 'linux-cachyos-git' -> master branch
        assert_eq!(
            classify_category("linux-cachyos-git").display(),
            "master branch"
        );
        // 'zen' inside 'linux-zen'
        assert_eq!(classify_category("linux-zen").display(), "zen-kernel");
    }

    #[test]
    fn needle_matches_oracle_regex() {
        assert!(matches_headers_needle("linux-cachyos-headers"));
        assert!(matches_headers_needle("linux-lts-headers"));
        assert!(matches_headers_needle("linux-headers"));
        assert!(matches_headers_needle("linux-api-headers"));
        assert!(matches_headers_needle("pre-linux-foo-headers"));
        // unanchored regex: substring match wins even though the name does
        // not end with -headers
        assert!(matches_headers_needle("linux-foo-headers-bar"));
        // the literal `-headers` matches the PREFIX of `-headersx` — the
        // unanchored regex matches "linux-cachyos-headersx" too (the oracle
        // then strips -headers and fails to find kernel `linux-cachyosx`, so
        // the row is dropped later)
        assert!(matches_headers_needle("linux-cachyos-headersx"));
        assert!(!matches_headers_needle("linux-cachyos"));
        assert!(!matches_headers_needle("headers-linux"));
        assert!(!matches_headers_needle("linux-foo headers"));
        assert!(!matches_headers_needle("cachyos"));
    }

    #[test]
    fn headers_to_kernel_removes_all_occurrences() {
        assert_eq!(
            kernel_headers_name("linux-cachyos-headers"),
            "linux-cachyos"
        );
        assert_eq!(kernel_headers_name("linux-lts-headers"), "linux-lts");
        // remove_all removes every occurrence, faithful to the oracle
        assert_eq!(kernel_headers_name("a-headers-b-headers"), "a-b");
    }

    #[test]
    fn api_headers_excluded() {
        assert!(is_api_headers("linux-api-headers"));
        // substring semantics: only names containing the literal
        // `linux-api-headers` are excluded (kernel.cpp:201-204); a name like
        // linux-cachyos-api-headers does NOT contain that substring and is
        // not excluded here (it is dropped later when the kernel lookup
        // fails)
        assert!(!is_api_headers("linux-cachyos-api-headers"));
        assert!(!is_api_headers("linux-cachyos-headers"));
    }

    #[test]
    fn version_display_matrix() {
        let d = DisplayVersion::compute(None, "6.14.1-1", vcmp);
        assert_eq!(d.text, "6.14.1-1");
        assert!(!d.update);

        let d = DisplayVersion::compute(Some("6.14.1-1"), "6.14.1-1", vcmp);
        assert_eq!(d.text, "6.14.1-1");
        assert!(!d.update);

        let d = DisplayVersion::compute(Some("6.13.9-1"), "6.14.1-1", vcmp);
        assert_eq!(d.text, format!("{UPDATE_MARKER}6.14.1-1"));
        assert!(d.update);

        let d = DisplayVersion::compute(Some("6.15.0-1"), "6.14.1-1", vcmp);
        assert_eq!(d.text, format!("{DOWNGRADE_MARKER}6.15.0-1"));
        assert!(!d.update);

        let d = DisplayVersion::for_aur();
        assert_eq!(d.text, "unknown-version");
        assert!(!d.update);
    }

    #[test]
    fn marker_strip_and_sort_key() {
        assert_eq!(strip_version_marker("∨6.15.0-1"), "6.15.0-1");
        assert_eq!(strip_version_marker("∧6.14.1-1"), "6.14.1-1");
        assert_eq!(strip_version_marker("6.14.1-1"), "6.14.1-1");
        assert_eq!(strip_version_marker("unknown-version"), "unknown-version");
    }

    #[test]
    fn kernel_name_newtype() {
        assert_eq!(
            KernelName::new("linux-cachyos").unwrap().as_str(),
            "linux-cachyos"
        );
        assert!(KernelName::new("cachyos/linux-cachyos").is_none());
        assert!(KernelName::new("").is_none());
    }
}
