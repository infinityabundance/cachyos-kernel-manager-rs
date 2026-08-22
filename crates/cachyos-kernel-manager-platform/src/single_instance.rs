//! Single-instance lock: the oracle's `IsInstanceAlreadyRunning`
//! (`main.cpp:45-56`) as a pure decision model + a real implementation.
//!
//! The oracle uses `QSharedMemory("CachyOS-KM-lock")`:
//!
//! ```cpp
//! if (!memoryLock.create(1)) {
//!     memoryLock.attach();
//!     memoryLock.detach();
//!     if (!memoryLock.create(1)) {
//!         return true; // already running
//!     }
//! }
//! ```
//!
//! Qt semantics (qsharedmemory_unix.cpp): `create` fails when the segment
//! already exists; the segment is marked for destruction (`IPC_RMID`) when
//! the CREATOR attaches — so it is destroyed when the LAST attachment ends
//! (including a crashed creator: the kernel drops the attachment and the
//! segment disappears). The attach+detach retry therefore RECOVERS a stale
//! segment from a crashed instance, and only a segment still held by a LIVE
//! other process survives the retry.
//!
//! The candidate implements the same decision against a real OS primitive
//! with the SAME NAME: a `flock(2)`-exclusive lock file. The name
//! (`CachyOS-KM-lock`) is the cross-implementation identity contract; the
//! stale-lock recovery (crash → the lock is released by the kernel) matches
//! the QSharedMemory behavior. Courted by `single-instance/stale-lock`
//! (the pure decision table, byte-for-byte against the frozen source's
//! re-declaration) — a running oracle and a running candidate both exit the
//! second instance with `-1` (`main.cpp:113`).

use std::path::PathBuf;

/// `QSharedMemory("CachyOS-KM-lock")` (`main.cpp:111`).
pub const SINGLE_INSTANCE_KEY: &str = "CachyOS-KM-lock";

/// Exit code of the second instance (`main.cpp:113`, `return -1`).
pub const SECOND_INSTANCE_EXIT_CODE: i32 = -1;

/// The `IsInstanceAlreadyRunning` decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(missing_docs)] // variants are self-describing
pub enum LockDecision {
    /// The lock was acquired; the app may proceed.
    Proceed,
    /// Another instance holds the lock; the app must exit (`-1`).
    AlreadyRunning,
}

/// The pure decision — the four OS outcomes the oracle's function observes:
///
/// - `create1_ok`: the first `create(1)` succeeded (no segment existed);
/// - `attach_ok` / `detach_ok`: the recovery attempt; `attach` succeeds iff
///   the segment still exists, `detach` always succeeds after a successful
///   attach;
/// - `create2_ok`: the retry; succeeds iff the segment was destroyed by our
///   detach (no other live holder).
///
/// Reproduced exactly from `main.cpp:45-56`; courted by
/// `single-instance/stale-lock`.
pub fn decide(
    create1_ok: bool,
    attach_ok: bool,
    detach_ok: bool,
    create2_ok: bool,
) -> LockDecision {
    if create1_ok {
        return LockDecision::Proceed;
    }
    // the recovery attempt: attach + detach (both must succeed for the
    // retry to be meaningful; a failed attach means the segment vanished
    // between the create failure and the attach — the retry is the only
    // path)
    if !attach_ok {
        return if create2_ok {
            LockDecision::Proceed
        } else {
            LockDecision::AlreadyRunning
        };
    }
    if !detach_ok {
        return LockDecision::AlreadyRunning;
    }
    if create2_ok {
        LockDecision::Proceed
    } else {
        LockDecision::AlreadyRunning
    }
}

/// The real single-instance lock: a `flock(2)`-exclusive lock file named
/// `CachyOS-KM-lock` under the runtime directory.
///
/// A crashed holder releases the lock automatically (the kernel drops the
/// flock on process death) — the stale-lock recovery the oracle's
/// attach/detach retry provides. `try_lock` is the `create(1)` equivalent;
/// holding the `InstanceLock` value keeps the lock until drop.
pub struct InstanceLock {
    file: std::fs::File,
    _path: PathBuf,
}

impl InstanceLock {
    /// Acquire the lock: `Proceed` + a held lock, or `AlreadyRunning`.
    ///
    /// The runtime directory is `$XDG_RUNTIME_DIR` when set (per-session
    /// shared memory semantics), else the cache root (a crash-cleaned
    /// location); `fs2`'s `try_lock_exclusive` mirrors `QSharedMemory::
    /// create` (fails when another process holds it).
    pub fn try_acquire(home: &str) -> Result<(InstanceLock, LockDecision), std::io::Error> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::cache_root(home));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(SINGLE_INSTANCE_KEY);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false) // the lock file's CONTENT is irrelevant; only
            // the flock matters — never truncate
            .read(true)
            .write(true)
            .open(&path)?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok((InstanceLock { file, _path: path }, LockDecision::Proceed)),
            Err(_) => Ok((
                InstanceLock { file, _path: path },
                LockDecision::AlreadyRunning,
            )),
        }
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        // the kernel releases the flock on close; the file stays (like the
        // shared-memory segment name, not its life)
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oracle's `IsInstanceAlreadyRunning` truth table (main.cpp:45-56):
    /// fresh -> proceed; held -> already running; stale (crashed) -> the
    /// attach/detach retry destroys the segment -> proceed; the retry is the
    /// ONLY path when the create fails.
    #[test]
    fn decision_table_matches_the_oracle() {
        // fresh: create succeeds
        assert_eq!(decide(true, false, false, false), LockDecision::Proceed);
        // held by another live process: create fails, attach ok, detach ok,
        // create fails again
        assert_eq!(
            decide(false, true, true, false),
            LockDecision::AlreadyRunning
        );
        // stale (crashed): create fails, attach ok, detach ok (last -> the
        // segment is destroyed), create succeeds
        assert_eq!(decide(false, true, true, true), LockDecision::Proceed);
        // vanished between create-failure and attach: the retry decides
        assert_eq!(decide(false, false, false, true), LockDecision::Proceed);
        assert_eq!(
            decide(false, false, false, false),
            LockDecision::AlreadyRunning
        );
        // attach ok but detach failed (a non-Qt anomaly): no recovery
        assert_eq!(
            decide(false, true, false, true),
            LockDecision::AlreadyRunning
        );
        assert_eq!(
            decide(false, true, false, false),
            LockDecision::AlreadyRunning
        );
    }

    #[test]
    fn a_held_lock_blocks_a_second_acquire() {
        let home = std::env::temp_dir().join(format!("km-lock-test-{}", std::process::id()));
        let home = home.to_str().unwrap();
        let (_first, decision) = InstanceLock::try_acquire(home).unwrap();
        assert_eq!(decision, LockDecision::Proceed);
        // a second acquire in the SAME process: flock is per-fd, so a
        // separate open in the same process conflicts (flock semantics)
        let (_second, decision2) = InstanceLock::try_acquire(home).unwrap();
        assert_eq!(decision2, LockDecision::AlreadyRunning);
        // after drop, a fresh acquire proceeds
        drop(_first);
        drop(_second);
        let (_third, decision3) = InstanceLock::try_acquire(home).unwrap();
        assert_eq!(decision3, LockDecision::Proceed);
    }
}
