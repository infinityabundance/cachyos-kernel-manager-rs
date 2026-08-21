//! Platform facts: cache paths, helper paths, home expansion, and the
//! single-instance lock identity — all reconstructed from the frozen oracle
//! (`src/utils.cpp`, `src/main.cpp`).
//!
//! These paths are part of the drop-in compatibility contract
//! (docs/COMPATIBILITY.md). Changing them silently would break existing
//! caches and automation.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Single-instance lock key: `QSharedMemory("CachyOS-KM-lock")`
/// (`main.cpp:111`). The candidate must use an equivalent named-lock identity
/// so that a running oracle and a running candidate cannot both execute
/// package transactions (court: `single-instance/*`).
pub const SINGLE_INSTANCE_KEY: &str = "CachyOS-KM-lock";

/// Exit code of the second instance (`main.cpp:113`, `return -1`).
pub const SECOND_INSTANCE_EXIT_CODE: i32 = -1;

/// Cache root: `~/.cache/cachyos-km` (`utils.cpp:199`).
pub fn cache_root(home: &str) -> PathBuf {
    PathBuf::from(home).join(".cache").join("cachyos-km")
}

/// Repository PKGBUILD cache: `~/.cache/cachyos-km/pkgbuilds`
/// (`utils.cpp:200`).
pub fn pkgbuilds_dir(home: &str) -> PathBuf {
    cache_root(home).join("pkgbuilds")
}

/// AUR PKGBUILD cache root: `~/.cache/cachyos-km/aur_pkgbuilds`
/// (`aur_kernel.cpp:33`).
pub fn aur_pkgbuilds_dir(home: &str) -> PathBuf {
    cache_root(home).join("aur_pkgbuilds")
}

/// linux-cachyos clone URL (`utils.cpp:201`).
pub const LINUX_CACHYOS_GIT_URL: &str = "https://github.com/cachyos/linux-cachyos.git";

/// AUR clone URL template (`aur_kernel.cpp:35`).
pub fn aur_git_url(package: &str) -> String {
    format!("https://aur.archlinux.org/{package}.git")
}

/// Helper install directory (`CMakeLists.txt:159-165`).
pub const HELPER_DIR: &str = "/usr/lib/cachyos-kernel-manager";

/// `fix_path` (`utils.cpp:153-159`): replace **all** `~` occurrences with
/// the home directory (glib `g_get_home_dir`, which honors `$HOME`).
/// The oracle's `path[0]` access on an empty string is UB and is
/// intentionally not reproduced (D-005); an empty path is returned as-is.
pub fn fix_path(path: &str, home: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    path.replace('~', home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_match_oracle_contract() {
        assert_eq!(
            cache_root("/home/u"),
            PathBuf::from("/home/u/.cache/cachyos-km")
        );
        assert_eq!(
            pkgbuilds_dir("/home/u"),
            PathBuf::from("/home/u/.cache/cachyos-km/pkgbuilds")
        );
        assert_eq!(
            aur_pkgbuilds_dir("/home/u"),
            PathBuf::from("/home/u/.cache/cachyos-km/aur_pkgbuilds")
        );
        assert_eq!(
            aur_git_url("linux-cachyos"),
            "https://aur.archlinux.org/linux-cachyos.git"
        );
        assert_eq!(SINGLE_INSTANCE_KEY, "CachyOS-KM-lock");
        assert_eq!(SECOND_INSTANCE_EXIT_CODE, -1);
    }

    #[test]
    fn fix_path_replaces_all_tildes() {
        assert_eq!(fix_path("~/x", "/home/u"), "/home/u/x");
        assert_eq!(fix_path("a~/b~", "/home/u"), "a/home/u/b/home/u");
        assert_eq!(fix_path("no-tilde", "/home/u"), "no-tilde");
        assert_eq!(fix_path("", "/home/u"), "");
    }
}
