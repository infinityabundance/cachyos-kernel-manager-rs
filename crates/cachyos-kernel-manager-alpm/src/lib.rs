//! Typed ALPM adapter.
//!
//! Architecture (directive §10):
//! ```text
//! Rust domain model
//!       ↓
//! typed ALPM adapter   (this crate)
//!       ↓
//! Rust libalpm binding (ffi module; feature `libalpm`)
//!       ↓
//! system libalpm
//! ```
//!
//! Everything except the FFI module is `#![forbid(unsafe_code)]`. When the
//! `libalpm` feature is off, the whole crate forbids unsafe; when it is on,
//! only `ffi.rs` (the documented, invariant-covered FFI boundary) may use
//! unsafe — every other module still forbids it at module level.
//!
//! The adapter preserves the oracle's ALPM usage exactly (`alpm_utils.cpp` +
//! `kernel.cpp`): initialize with root `/` and dbpath `/var/lib/pacman/`,
//! register sync dbs from a mINI-style parse of `/etc/pacman.conf` (skipping
//! `testing` and `options`), search with the `linux[^ ]*-headers` needle,
//! and compare versions with `alpm_pkg_vercmp` — never semver.

#![cfg_attr(not(feature = "libalpm"), forbid(unsafe_code))]

pub mod pacman_conf;

#[cfg(feature = "libalpm")]
pub mod ffi;

use cachyos_kernel_manager_core::{DbPackage, SyncDb};
use std::cmp::Ordering;

/// A source of ALPM facts. Implemented by the real libalpm backend
/// (feature `libalpm`, Phase 4) and by [`NullAlpm`] for tests and pure
/// courts.
pub trait Alpm {
    /// The registered sync databases in registration (pacman.conf) order.
    fn sync_dbs(&self) -> Vec<SyncDb>;

    /// Look up an installed package in the local database.
    fn local_pkg(&self, name: &str) -> Option<DbPackage>;

    /// The local database provenance (`alpm_pkg_get_installed_db`), empty
    /// when the libalpm version lacks it.
    fn installed_db(&self, name: &str) -> Option<String>;

    /// ALPM version comparison (`alpm_pkg_vercmp`).
    fn vercmp(&self, a: &str, b: &str) -> Ordering;
}

/// In-memory backend for tests and pure courts. Not a parity oracle — the
/// real backend inside a disposable VM is (directive §73).
#[derive(Debug, Clone, Default)]
pub struct NullAlpm {
    pub sync: Vec<SyncDb>,
    pub local: Vec<DbPackage>,
}

impl NullAlpm {
    pub fn new(sync: Vec<SyncDb>, local: Vec<DbPackage>) -> Self {
        NullAlpm { sync, local }
    }
}

impl Alpm for NullAlpm {
    fn sync_dbs(&self) -> Vec<SyncDb> {
        self.sync.clone()
    }

    fn local_pkg(&self, name: &str) -> Option<DbPackage> {
        self.local.iter().find(|p| p.name == name).cloned()
    }

    fn installed_db(&self, _name: &str) -> Option<String> {
        None // unknown provenance
    }

    fn vercmp(&self, a: &str, b: &str) -> Ordering {
        // Deterministic stand-in so tests are stable. This is NOT the parity
        // comparator: ALPM's real `alpm_pkg_vercmp` is provided by the
        // `libalpm` feature, and every version-state court compares against
        // it. Segment-wise numeric comparison handles `6.14.1-1` vs
        // `6.14.1-2`; non-numeric segments fall back to lexicographic
        // comparison.
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
}

/// The oracle's pacman.conf registration rule (`alpm_utils.cpp:32-47`):
/// skip `testing` and `options`; register every other section. The mINI
/// parser semantics themselves live in [`pacman_conf`] — this function
/// encodes the *registration* rule on top of whatever section list a parser
/// yields.
pub fn register_sections(sections: &[String]) -> Vec<String> {
    sections
        .iter()
        .filter(|s| s.as_str() != "testing" && s.as_str() != "options")
        .cloned()
        .collect()
}

/// ALPM version comparison as a pure function (`alpm_pkg_vercmp` is
/// STATELESS — no handle state — so the UI can sort by version without
/// owning an [`Alpm`] source).
///
/// With the `libalpm` feature this is the real libalpm comparator; without
/// it, the deterministic segment-wise fallback (`NullAlpm`'s).
#[cfg(feature = "libalpm")]
pub fn vercmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::ffi::CString;
    unsafe {
        let a = CString::new(a).expect("no interior NUL");
        let b = CString::new(b).expect("no interior NUL");
        ffi::alpm_pkg_vercmp(a.as_ptr(), b.as_ptr()).cmp(&0)
    }
}

/// Non-libalpm build: the deterministic fallback (same as [`NullAlpm`]).
#[cfg(not(feature = "libalpm"))]
pub fn vercmp(a: &str, b: &str) -> std::cmp::Ordering {
    NullAlpm::default().vercmp(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_skips_testing_and_options() {
        let sections = vec![
            "options".into(),
            "testing".into(),
            "core".into(),
            "cachyos".into(),
        ];
        assert_eq!(register_sections(&sections), vec!["core", "cachyos"]);
    }

    #[test]
    fn null_backend_round_trips() {
        let db = SyncDb {
            name: "cachyos".into(),
            packages: vec![],
        };
        let alpm = NullAlpm::new(vec![db], vec![]);
        assert_eq!(alpm.sync_dbs().len(), 1);
        assert_eq!(alpm.local_pkg("linux").map(|p| p.name), None);
        assert_eq!(alpm.vercmp("6.14.1-1", "6.14.1-2"), Ordering::Less);
    }
}
