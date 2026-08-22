//! Minimal, isolated libalpm FFI (Phase 4).
//!
//! This is the ONLY module in the workspace that may contain `unsafe`.
//! Invariants that make it sound:
//!
//! 1. Handles are opaque pointers; every handle created by [`AlpmHandle::init`]
//!    is released exactly once via [`Drop`] (RAII), and all package/db
//!    pointers derived from it are copied into owned Rust values immediately,
//!    never retained across calls.
//! 2. libalpm strings (`const char*`) are owned by libalpm and valid while
//!    the handle lives; we copy them into Rust `String` right away.
//! 3. `alpm_list_t` lists are traversed in place and never retained.
//! 4. Thread discipline: libalpm handles are NOT thread-safe. An
//!    [`AlpmHandle`] is neither [`Send`] nor [`Sync`] (raw pointer field;
//!    no unsafe impls) — the compiler proves it cannot leave the creating
//!    thread. The inspect/plan tools are single-threaded by construction.
//! 5. `ALPM_SIG_USE_DEFAULT` is `(1 << 30)` on every libalpm ≥ 13 (the
//!    oracle requires `libalpm>=13.0.0`); the value, every `extern "C"`
//!    signature, and the `alpm_list_t` layout are machine-verified against
//!    the system headers at build time by `abi/probe.c` (compiled and run
//!    from `build.rs`; court `alpm-ffi/abi-surface`).
//!
//! The adapter never calls into libalpm's transaction API: pacman remains
//! the mutation authority (directive §16). This layer is read-only package
//! state.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

/// `ALPM_SIG_USE_DEFAULT = (1 << 30)` (alpm.h:404). Machine-verified by
/// abi/probe.c at build time + court alpm-ffi/abi-surface.
pub const ALPM_SIG_USE_DEFAULT: c_int = 1 << 30;

/// libalpm's linked list (`alpm_list_t`): data + prev + next.
/// alpm_list.h: `struct _alpm_list_t { void *data; struct _alpm_list_t
/// *prev; struct _alpm_list_t *next; }` — the PREV field must be present so
/// `next` is read at the correct offset (16, not 8). Machine-verified by
/// abi/probe.c at build time + court alpm-ffi/abi-surface.
#[repr(C)]
struct RawList {
    data: *mut c_void,
    prev: *mut RawList,
    next: *mut RawList,
}

/// The Rust-side ABI facts, printed by the `cachyos-kernel-manager-alpm-abi`
/// witness (court `alpm-ffi/abi-surface`) in the exact `abi/probe.c` format.
pub const RAW_LIST_SIZE: usize = std::mem::size_of::<RawList>();
pub const RAW_LIST_OFFSET_DATA: usize = std::mem::offset_of!(RawList, data);
pub const RAW_LIST_OFFSET_PREV: usize = std::mem::offset_of!(RawList, prev);
pub const RAW_LIST_OFFSET_NEXT: usize = std::mem::offset_of!(RawList, next);
pub const PTR_SIZE: usize = std::mem::size_of::<*const c_void>();
pub const ENUM_SIZE: usize = std::mem::size_of::<c_int>();

/// `alpm_handle_t` (opaque).
#[repr(C)]
struct RawHandle {
    _private: [u8; 0],
}

/// `alpm_db_t` (opaque).
#[repr(C)]
struct RawDb {
    _private: [u8; 0],
}

/// `alpm_pkg_t` (opaque).
#[repr(C)]
struct RawPkg {
    _private: [u8; 0],
}

extern "C" {
    fn alpm_initialize(
        root: *const c_char,
        dbpath: *const c_char,
        err: *mut c_int,
    ) -> *mut RawHandle;
    fn alpm_release(handle: *mut RawHandle) -> c_int;
    fn alpm_errno(handle: *mut RawHandle) -> c_int;
    fn alpm_strerror(err: c_int) -> *const c_char;
    fn alpm_register_syncdb(
        handle: *mut RawHandle,
        treename: *const c_char,
        level: c_int,
    ) -> *mut RawDb;
    fn alpm_get_syncdbs(handle: *mut RawHandle) -> *mut RawList;
    fn alpm_get_localdb(handle: *mut RawHandle) -> *mut RawDb;
    // the header declares `const alpm_db_t *` (alpm.h:1291); the pointer
    // constness is ABI-neutral but the signature check in abi/probe.c
    // requires the declaration to match exactly
    fn alpm_db_get_name(db: *const RawDb) -> *const c_char;
    fn alpm_db_get_pkg(db: *mut RawDb, name: *const c_char) -> *mut RawPkg;
    fn alpm_db_get_pkgcache(db: *mut RawDb) -> *mut RawList;
    fn alpm_pkg_get_name(pkg: *mut RawPkg) -> *const c_char;
    fn alpm_pkg_get_version(pkg: *mut RawPkg) -> *const c_char;
    // NOTE: this CachyOS-patched API returns the installed database NAME as
    // a `const char*` (alpm.h:2560), NOT an `alpm_db_t*` — the oracle stores
    // it as a string (`kernel.cpp:220-224`). Declaring it as `*mut RawDb`
    // would call alpm_db_get_name on a string pointer (garbage).
    fn alpm_pkg_get_installed_db(pkg: *mut RawPkg) -> *const c_char;
    fn alpm_pkg_vercmp(a: *const c_char, b: *const c_char) -> c_int;
}

unsafe fn cstr(v: &str) -> CString {
    CString::new(v).expect("no interior NUL in paths/names")
}

unsafe fn c_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}

/// Errors surfaced from libalpm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlpmError {
    /// `alpm_initialize` returned null; carries the errno text.
    Init(String),
    /// `alpm_release` returned nonzero.
    Release(String),
}

impl std::fmt::Display for AlpmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlpmError::Init(e) => write!(f, "alpm init failed: {e}"),
            AlpmError::Release(e) => write!(f, "alpm release failed: {e}"),
        }
    }
}

impl std::error::Error for AlpmError {}

/// A RAII-guarded read-only libalpm handle.
///
/// Thread discipline: NOT [`Send`], NOT [`Sync`]. The raw pointer field
/// makes this automatic — there is deliberately NO `unsafe impl` — so the
/// compiler proves the handle never leaves the thread that created it
/// (invariant 4 above).
pub struct AlpmHandle {
    raw: *mut RawHandle,
}

impl AlpmHandle {
    /// `alpm_initialize(root, dbpath, &err)` — mirrors the oracle's
    /// `parse_alpm("/", "/var/lib/pacman/", ...)` (`alpm_utils.cpp:26`).
    pub fn init(root: &str, dbpath: &str) -> Result<AlpmHandle, AlpmError> {
        unsafe {
            let root = cstr(root);
            let dbpath = cstr(dbpath);
            let mut err: c_int = 0;
            let raw = alpm_initialize(root.as_ptr(), dbpath.as_ptr(), &mut err);
            if raw.is_null() {
                let msg = c_to_string(alpm_strerror(err)).unwrap_or_else(|| "unknown".to_string());
                return Err(AlpmError::Init(msg));
            }
            Ok(AlpmHandle { raw })
        }
    }

    /// The last libalpm error string (errno → strerror, oracle parity:
    /// `alpm_errno` is read before release in `alpm_utils.cpp:52-59`).
    pub fn last_error(&self) -> String {
        unsafe {
            let errno = alpm_errno(self.raw);
            if errno == 0 {
                return "no error".to_string();
            }
            c_to_string(alpm_strerror(errno)).unwrap_or_else(|| "unknown error".to_string())
        }
    }

    /// Register a sync database with `ALPM_SIG_USE_DEFAULT` (oracle parity).
    pub fn register_syncdb(&self, name: &str) {
        unsafe {
            let name = cstr(name);
            alpm_register_syncdb(self.raw, name.as_ptr(), ALPM_SIG_USE_DEFAULT);
        }
    }

    /// Sync database names, in registration order (oracle parity:
    /// `alpm_get_syncdbs`).
    pub fn syncdb_names(&self) -> Vec<String> {
        unsafe {
            let mut out = Vec::new();
            let mut list = alpm_get_syncdbs(self.raw);
            while !list.is_null() {
                let db = (*list).data as *mut RawDb;
                if let Some(name) = c_to_string(alpm_db_get_name(db)) {
                    out.push(name);
                }
                list = (*list).next;
            }
            out
        }
    }

    /// Packages of a sync database in database order (`alpm_db_get_pkgcache`).
    pub fn db_packages(&self, db_name: &str) -> Vec<DbPkg> {
        unsafe {
            let mut out = Vec::new();
            let db = self.db_by_name(db_name);
            if db.is_null() {
                return out;
            }
            let mut list = alpm_db_get_pkgcache(db);
            while !list.is_null() {
                let pkg = (*list).data as *mut RawPkg;
                if let (Some(name), Some(version)) = (
                    c_to_string(alpm_pkg_get_name(pkg)),
                    c_to_string(alpm_pkg_get_version(pkg)),
                ) {
                    out.push(DbPkg { name, version });
                }
                list = (*list).next;
            }
            out
        }
    }

    /// Enumerate the LOCAL database (every installed package). Used by the
    /// plan tool for the oracle's local-db lookups (`kernel.cpp:102-109`:
    /// `nvidia-dkms`/`nvidia-open-dkms` presence; `kernel.cpp:143-161`:
    /// removal companions must be installed).
    pub fn local_packages(&self) -> Vec<DbPkg> {
        unsafe {
            let mut out = Vec::new();
            let db = alpm_get_localdb(self.raw);
            if db.is_null() {
                return out;
            }
            let mut list = alpm_db_get_pkgcache(db);
            while !list.is_null() {
                let pkg = (*list).data as *mut RawPkg;
                if let (Some(name), Some(version)) = (
                    c_to_string(alpm_pkg_get_name(pkg)),
                    c_to_string(alpm_pkg_get_version(pkg)),
                ) {
                    out.push(DbPkg { name, version });
                }
                list = (*list).next;
            }
            out
        }
    }

    /// Look up a package in a sync database by name.
    pub fn db_get_pkg(&self, db_name: &str, pkg_name: &str) -> Option<DbPkg> {
        unsafe {
            let db = self.db_by_name(db_name);
            if db.is_null() {
                return None;
            }
            let name = cstr(pkg_name);
            let pkg = alpm_db_get_pkg(db, name.as_ptr());
            if pkg.is_null() {
                return None;
            }
            Some(DbPkg {
                name: c_to_string(alpm_pkg_get_name(pkg)).unwrap_or_default(),
                version: c_to_string(alpm_pkg_get_version(pkg)).unwrap_or_default(),
            })
        }
    }

    /// Installed package (local database) by name.
    pub fn local_pkg(&self, pkg_name: &str) -> Option<LocalPkg> {
        unsafe {
            let db = alpm_get_localdb(self.raw);
            if db.is_null() {
                return None;
            }
            let name = cstr(pkg_name);
            let pkg = alpm_db_get_pkg(db, name.as_ptr());
            if pkg.is_null() {
                return None;
            }
            // alpm_pkg_get_installed_db returns the provenance db NAME as a
            // C string, or NULL for packages whose origin is unknown (e.g.
            // installed by file); the oracle guards this with
            // HAVE_ALPM_INSTALLED_DB.
            let installed_db = {
                let name = alpm_pkg_get_installed_db(pkg);
                if name.is_null() {
                    None
                } else {
                    c_to_string(name)
                }
            };
            Some(LocalPkg {
                name: c_to_string(alpm_pkg_get_name(pkg)).unwrap_or_default(),
                version: c_to_string(alpm_pkg_get_version(pkg)).unwrap_or_default(),
                installed_db,
            })
        }
    }

    /// ALPM version comparison (`alpm_pkg_vercmp`): -1 / 0 / 1.
    pub fn vercmp(&self, a: &str, b: &str) -> i32 {
        unsafe {
            let a = cstr(a);
            let b = cstr(b);
            alpm_pkg_vercmp(a.as_ptr(), b.as_ptr())
        }
    }

    fn db_by_name(&self, db_name: &str) -> *mut RawDb {
        unsafe {
            let mut list = alpm_get_syncdbs(self.raw);
            while !list.is_null() {
                let db = (*list).data as *mut RawDb;
                if let Some(name) = c_to_string(alpm_db_get_name(db)) {
                    if name == db_name {
                        return db;
                    }
                }
                list = (*list).next;
            }
            std::ptr::null_mut()
        }
    }
}

impl Drop for AlpmHandle {
    fn drop(&mut self) {
        unsafe {
            alpm_release(self.raw);
        }
    }
}

/// A package as seen through libalpm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbPkg {
    pub name: String,
    pub version: String,
}

/// An installed package with provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPkg {
    pub name: String,
    pub version: String,
    /// `alpm_pkg_get_installed_db` provenance (None when unknown).
    pub installed_db: Option<String>,
}
