//! Kernel discovery semantics, reconstructed from `Kernel::get_kernels`
//! (`oracle/upstream/src/kernel.cpp:179-286`).
//!
//! The oracle iterates the registered sync databases in pacman.conf section
//! order, searches each with the libalpm regex needle `linux[^ ]*-headers`,
//! pairs each headers package with its kernel package **in the same
//! database**, and attaches companion module names (ZFS / NVIDIA /
//! NVIDIA-open) that also exist in that database.
//!
//! This module is the pure model: the ALPM crate feeds it real database
//! contents; tests feed it fixtures. Protected by courts
//! `kernel-discovery/*`.

use crate::kernel::{is_api_headers, kernel_headers_name, matches_headers_needle};

/// One package as seen in a sync database (or local database).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DbPackage {
    /// Package name, e.g. `linux-cachyos` or `linux-cachyos-headers`.
    pub name: String,
    /// Package version string, e.g. `6.14.1-1`.
    pub version: String,
}

/// One sync database (a `[repo]` section of pacman.conf as registered by the
/// oracle's mINI parser).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncDb {
    /// Database/repository name (the pacman.conf section name).
    pub name: String,
    /// Packages in the database, in database order.
    pub packages: Vec<DbPackage>,
}

/// Companion module package names that may accompany a kernel, per the
/// oracle's naming rules (`kernel.cpp:226-244`). Names are only *candidates*;
/// existence in the same database is checked separately.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompanionNames {
    /// e.g. `linux-cachyos-zfs`.
    pub zfs: Option<String>,
    /// e.g. `linux-cachyos-nvidia` or `nvidia` / `nvidia-lts`.
    pub nvidia: Option<String>,
    /// e.g. `linux-cachyos-nvidia-open` or `nvidia-open` / `nvidia-open-lts`.
    pub nvidia_open: Option<String>,
}

/// Compute the oracle's companion *naming* rules for a kernel name.
///
/// - `linux-cachyos*`: `{name}-zfs`, `{name}-nvidia`, `{name}-nvidia-open`.
/// - `linux` / `linux-lts`: all `linux` substrings removed from the name,
///   then `nvidia{stripped}` / `nvidia-open{stripped}`.
///   (`remove_all(kernel_module, "linux")` removes every occurrence.)
/// - anything else: no companions.
pub fn companions_for(kernel_name: &str) -> CompanionNames {
    if kernel_name.starts_with("linux-cachyos") {
        CompanionNames {
            zfs: Some(format!("{kernel_name}-zfs")),
            nvidia: Some(format!("{kernel_name}-nvidia")),
            nvidia_open: Some(format!("{kernel_name}-nvidia-open")),
        }
    } else if kernel_name == "linux" || kernel_name == "linux-lts" {
        let stripped = kernel_name.replace("linux", "");
        CompanionNames {
            zfs: None,
            nvidia: Some(format!("nvidia{stripped}")),
            nvidia_open: Some(format!("nvidia-open{stripped}")),
        }
    } else {
        CompanionNames::default()
    }
}

/// A kernel discovered in a sync database, mirroring the oracle's `Kernel`
/// object fields that matter for the UI and transaction planning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredKernel {
    /// Repository (database) name.
    pub repo: String,
    /// Kernel package name (headers suffix removed).
    pub name: String,
    /// Headers package name (the package that matched the search needle).
    pub headers: String,
    /// Version from the sync package.
    pub version: String,
    /// Companion candidates that exist in the same database.
    pub companions: CompanionNames,
    /// The oracle's display string: `<repo>/<kernel>`.
    pub raw: String,
}

/// Reconstruct `Kernel::get_kernels` as a pure function.
///
/// For each database, in order:
/// 1. find packages matching the needle `linux[^ ]*-headers`,
/// 2. drop those containing `linux-api-headers`,
/// 3. headers package = the match; kernel name = all `-headers` removed,
/// 4. kernel must exist in the same database, else the candidate is skipped,
/// 5. companion names resolved against the same database.
pub fn discover_kernels(dbs: &[SyncDb]) -> Vec<DiscoveredKernel> {
    let mut out = Vec::new();
    for db in dbs {
        let by_name = |name: &str| db.packages.iter().find(|p| p.name == name);
        for pkg in &db.packages {
            if !matches_headers_needle(&pkg.name) || is_api_headers(&pkg.name) {
                continue;
            }
            let headers = pkg.name.clone();
            let kernel_name = kernel_headers_name(&headers);
            let Some(kernel_pkg) = by_name(&kernel_name) else {
                // "Skip if the actual kernel package is not found" (same db)
                continue;
            };
            let names = companions_for(&kernel_name);
            let companions = CompanionNames {
                zfs: names.zfs.filter(|n| by_name(n).is_some()),
                nvidia: names.nvidia.filter(|n| by_name(n).is_some()),
                nvidia_open: names.nvidia_open.filter(|n| by_name(n).is_some()),
            };
            out.push(DiscoveredKernel {
                repo: db.name.clone(),
                name: kernel_name.clone(),
                headers,
                version: kernel_pkg.version.clone(),
                companions,
                raw: format!("{}/{}", db.name, kernel_name),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(name: &str, pkgs: &[&str]) -> SyncDb {
        SyncDb {
            name: name.to_string(),
            packages: pkgs
                .iter()
                .map(|p| DbPackage {
                    name: p.to_string(),
                    version: "1.0-1".to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn companion_naming_rules() {
        assert_eq!(
            companions_for("linux-cachyos"),
            CompanionNames {
                zfs: Some("linux-cachyos-zfs".into()),
                nvidia: Some("linux-cachyos-nvidia".into()),
                nvidia_open: Some("linux-cachyos-nvidia-open".into()),
            }
        );
        assert_eq!(
            companions_for("linux-cachyos-lts"),
            CompanionNames {
                zfs: Some("linux-cachyos-lts-zfs".into()),
                nvidia: Some("linux-cachyos-lts-nvidia".into()),
                nvidia_open: Some("linux-cachyos-lts-nvidia-open".into()),
            }
        );
        // linux -> nvidia / nvidia-open
        assert_eq!(
            companions_for("linux"),
            CompanionNames {
                zfs: None,
                nvidia: Some("nvidia".into()),
                nvidia_open: Some("nvidia-open".into()),
            }
        );
        // linux-lts -> nvidia-lts / nvidia-open-lts
        assert_eq!(
            companions_for("linux-lts"),
            CompanionNames {
                zfs: None,
                nvidia: Some("nvidia-lts".into()),
                nvidia_open: Some("nvidia-open-lts".into()),
            }
        );
        // not a cachyos/linux/linux-lts kernel -> no companions
        assert_eq!(companions_for("linux-zen"), CompanionNames::default());
        assert_eq!(
            companions_for("linux-cachyos-rc"),
            CompanionNames {
                zfs: Some("linux-cachyos-rc-zfs".into()),
                nvidia: Some("linux-cachyos-rc-nvidia".into()),
                nvidia_open: Some("linux-cachyos-rc-nvidia-open".into()),
            }
        );
    }

    #[test]
    fn discovery_pairs_kernel_with_headers_in_same_db() {
        let dbs = vec![db("cachyos", &["linux-cachyos", "linux-cachyos-headers"])];
        let kernels = discover_kernels(&dbs);
        assert_eq!(kernels.len(), 1);
        let k = &kernels[0];
        assert_eq!(k.repo, "cachyos");
        assert_eq!(k.name, "linux-cachyos");
        assert_eq!(k.headers, "linux-cachyos-headers");
        assert_eq!(k.raw, "cachyos/linux-cachyos");
    }

    #[test]
    fn discovery_skips_headers_without_kernel_in_same_db() {
        // headers exist but the kernel package is absent from THIS db
        let dbs = vec![db("custom", &["linux-cachyos-headers"])];
        assert!(discover_kernels(&dbs).is_empty());
    }

    #[test]
    fn discovery_excludes_api_headers() {
        let dbs = vec![db("core", &["linux-api-headers"])];
        assert!(discover_kernels(&dbs).is_empty());
    }

    #[test]
    fn discovery_ignores_non_kernel_linux_packages() {
        // "packages containing linux but not kernels"
        let dbs = vec![db("extra", &["linux-firmware"])];
        assert!(discover_kernels(&dbs).is_empty());
    }

    #[test]
    fn discovery_same_name_in_two_dbs_yields_two_rows() {
        // duplicate package names in multiple repositories -> one row per db
        let dbs = vec![
            db("cachyos", &["linux-cachyos", "linux-cachyos-headers"]),
            db("custom", &["linux-cachyos", "linux-cachyos-headers"]),
        ];
        let kernels = discover_kernels(&dbs);
        assert_eq!(kernels.len(), 2);
        assert_eq!(kernels[0].repo, "cachyos");
        assert_eq!(kernels[1].repo, "custom");
    }

    #[test]
    fn discovery_resolves_companions_only_in_same_db() {
        let dbs = vec![db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
                // nvidia exists but nvidia-open does not
                "linux-cachyos-nvidia",
            ],
        )];
        let kernels = discover_kernels(&dbs);
        assert_eq!(kernels.len(), 1);
        assert_eq!(kernels[0].companions.zfs, Some("linux-cachyos-zfs".into()));
        assert_eq!(
            kernels[0].companions.nvidia,
            Some("linux-cachyos-nvidia".into())
        );
        assert_eq!(kernels[0].companions.nvidia_open, None);
    }

    #[test]
    fn discovery_deduplicates_aur_style_names_not_present() {
        // kernel with no headers in any db is not discovered at all
        let dbs = vec![db("cachyos", &["linux-cachyos"])];
        assert!(discover_kernels(&dbs).is_empty());
    }

    #[test]
    fn discovery_preserves_db_order() {
        let dbs = vec![
            db("zrepo", &["linux-cachyos", "linux-cachyos-headers"]),
            db("arepo", &["linux-cachyos", "linux-cachyos-headers"]),
        ];
        let kernels = discover_kernels(&dbs);
        assert_eq!(kernels[0].repo, "zrepo");
        assert_eq!(kernels[1].repo, "arepo");
    }
}
