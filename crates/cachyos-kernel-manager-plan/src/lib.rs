//! Transaction planning.
//!
//! Reconstructs the oracle's package expansion (`Kernel::install`,
//! `Kernel::remove`, `commit_transaction`, `km-window.cpp:48-71`) as a pure,
//! inspectable object:
//!
//! ```text
//! USER INTENT → TRANSACTION PLAN → SYSTEM COMMAND → MACHINE SIDE EFFECT
//! ```
//!
//! Every implicit package carries a [`Reason`]. The plan is deterministic
//! for fixed input, which makes it courtable (`courts/transaction-plan/*`)
//! and property-testable.

#![forbid(unsafe_code)]

use cachyos_kernel_manager_core::selection::{KernelRow, SelectionState};
use cachyos_kernel_manager_core::DiscoveredKernel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Why a package is in the plan (directive §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    /// Directly selected by the user.
    SelectedKernel,
    /// The kernel's headers package.
    RequiredHeaders,
    /// Root filesystem is ZFS → the kernel's `-zfs` companion.
    ZfsRootCompanion,
    /// NVIDIA proprietary prebuilt companion.
    NvidiaCompanion,
    /// NVIDIA-open prebuilt companion.
    NvidiaOpenCompanion,
    /// The oracle's "modules already installed → reuse them" branch.
    ExistingModuleFamily,
    /// AUR kernel.
    AurDependency,
    /// Removal of an installed companion of a removed kernel.
    RemovalCompanion,
}

/// One planned package action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageAction {
    /// Package name as passed to pacman.
    pub package: String,
    /// Why it is planned.
    pub reason: Reason,
}

/// The inspectable plan (directive §13).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionPlan {
    /// Install actions in the oracle's order (`[zfs?, nvidia?, kernel,
    /// headers]` per selected kernel).
    pub install: Vec<PackageAction>,
    /// Remove actions in the oracle's order (`kernel, headers, zfs, nvidia,
    /// nvidia-open`, installed-only companions).
    pub remove: Vec<PackageAction>,
    /// Human-readable explanations for non-obvious expansions.
    pub warnings: Vec<String>,
}

/// Hardware/environment facts the expansion depends on
/// (`kernel.cpp:41-52` — evaluated once per process by the oracle).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// `findmnt -ln -o FSTYPE /` == `zfs`.
    pub root_on_zfs: bool,
    /// Any chwd profile name starts with `nvidia-dkms`.
    pub chwd_nvidia: bool,
    /// Any chwd profile name starts with `nvidia-open-dkms`.
    pub chwd_nvidia_open: bool,
    /// Installed package names (the local database).
    pub installed: BTreeSet<String>,
    /// Non-empty result of `pacman -Qqs '^linux-cachyos.*-nvidia$'`.
    pub nvidia_modules_installed: bool,
    /// Non-empty result of `pacman -Qqs '^linux-cachyos.*-nvidia-open$'`.
    pub nvidia_open_modules_installed: bool,
}

impl TransactionPlan {
    /// Build the plan from the current selection and environment.
    ///
    /// Mirrors the oracle flow: compute the shared change list, then the
    /// install phase (`install_packages` → `Kernel::install`) and removal
    /// phase (`remove_packages` → `Kernel::remove`).
    pub fn from_selection(
        selection: &SelectionState,
        hardware: &HardwareProfile,
        kernels_by_raw: &std::collections::BTreeMap<String, DiscoveredKernel>,
    ) -> TransactionPlan {
        let mut plan = TransactionPlan::default();

        for raw in selection.install_set() {
            let Some(kernel) = kernels_by_raw.get(&raw) else {
                continue;
            };
            plan.expand_install(kernel, hardware);
        }
        for raw in selection.removal_set() {
            let Some(kernel) = kernels_by_raw.get(&raw) else {
                continue;
            };
            plan.expand_remove(kernel, hardware);
        }
        plan
    }

    /// `Kernel::install` (`kernel.cpp:89-135`).
    pub fn expand_install(&mut self, kernel: &DiscoveredKernel, hardware: &HardwareProfile) {
        // ZFS companion first.
        if hardware.root_on_zfs {
            if let Some(zfs) = &kernel.companions.zfs {
                self.install.push(PackageAction {
                    package: zfs.clone(),
                    reason: Reason::ZfsRootCompanion,
                });
            }
        }

        let nvidia_dkms_installed = hardware.installed.contains("nvidia-dkms");
        let nvidia_open_dkms_installed = hardware.installed.contains("nvidia-open-dkms");
        let dkms_modules_not_installed = !nvidia_dkms_installed && !nvidia_open_dkms_installed;

        // "if we have any of the modules already installed, then just use
        // whatever is installed. skipping chwd detection"
        let mut should_install_nvidia = hardware.chwd_nvidia && kernel.companions.nvidia.is_some();
        let mut should_install_nvidia_open =
            hardware.chwd_nvidia_open && kernel.companions.nvidia_open.is_some();

        if hardware.nvidia_open_modules_installed && kernel.companions.nvidia_open.is_some() {
            should_install_nvidia_open = true;
            should_install_nvidia = false;
        } else if hardware.nvidia_modules_installed && kernel.companions.nvidia.is_some() {
            should_install_nvidia_open = false;
            should_install_nvidia = true;
        }

        let reason = if should_install_nvidia_open {
            Reason::NvidiaOpenCompanion
        } else {
            Reason::NvidiaCompanion
        };
        if dkms_modules_not_installed && should_install_nvidia_open {
            self.install.push(PackageAction {
                package: kernel
                    .companions
                    .nvidia_open
                    .clone()
                    .expect("checked above"),
                reason,
            });
        } else if dkms_modules_not_installed && should_install_nvidia {
            self.install.push(PackageAction {
                package: kernel.companions.nvidia.clone().expect("checked above"),
                reason,
            });
        }

        self.install.push(PackageAction {
            package: kernel.name.clone(),
            reason: Reason::SelectedKernel,
        });
        self.install.push(PackageAction {
            package: kernel.headers.clone(),
            reason: Reason::RequiredHeaders,
        });
    }

    /// `Kernel::remove` (`kernel.cpp:137-163`) + the commit ordering
    /// (kernel first, then installed companions).
    pub fn expand_remove(&mut self, kernel: &DiscoveredKernel, hardware: &HardwareProfile) {
        self.remove.push(PackageAction {
            package: kernel.name.clone(),
            reason: Reason::SelectedKernel,
        });
        // append_to_removal_list(headers) then the companion modules, each
        // only if installed locally (kernel.cpp:143-161).
        if hardware.installed.contains(&kernel.headers) {
            self.remove.push(PackageAction {
                package: kernel.headers.clone(),
                reason: Reason::RemovalCompanion,
            });
        }
        for companion in [
            &kernel.companions.zfs,
            &kernel.companions.nvidia,
            &kernel.companions.nvidia_open,
        ]
        .into_iter()
        .flatten()
        {
            if hardware.installed.contains(companion) {
                self.remove.push(PackageAction {
                    package: companion.clone(),
                    reason: Reason::RemovalCompanion,
                });
            }
        }
    }
}

/// Build the `raw → DiscoveredKernel` map the planner consumes.
pub fn kernels_by_raw(
    kernels: &[DiscoveredKernel],
) -> std::collections::BTreeMap<String, DiscoveredKernel> {
    kernels.iter().map(|k| (k.raw.clone(), k.clone())).collect()
}

/// Derive the oracle's tree-row flags from discovery + local db + installed
/// provenance. `installed_db` is the `HAVE_ALPM_INSTALLED_DB` provenance
/// (empty string when unknown).
pub fn to_rows(
    kernels: &[DiscoveredKernel],
    local_versions: &std::collections::BTreeMap<String, (String, String)>,
) -> Vec<KernelRow> {
    kernels
        .iter()
        .map(|k| {
            let installed = local_versions.contains_key(&k.name);
            let installed_db = local_versions
                .get(&k.name)
                .map(|(db, _)| db.as_str())
                .unwrap_or("");
            let immutable = installed && (installed_db.is_empty() || installed_db == k.repo);
            // version comparison is the alpm layer's job; the plan layer only
            // needs the update flag, computed by the caller.
            KernelRow {
                raw: k.raw.clone(),
                name: k.name.clone(),
                installed,
                immutable,
                update_available: false,
                checked: installed && immutable,
            }
        })
        .collect()
}

/// AUR selection support (`kernel.cpp:89-95`): selecting an AUR kernel adds
/// it to the aur install list without companions.
pub fn expand_aur_install(plan: &mut TransactionPlan, aur_kernel_name: &str) {
    plan.install.push(PackageAction {
        package: aur_kernel_name.to_string(),
        reason: Reason::AurDependency,
    });
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // test fixtures mutate defaults deliberately
mod tests {
    use super::*;
    use cachyos_kernel_manager_core::{discover_kernels, DbPackage, SyncDb};

    fn db(name: &str, pkgs: &[&str]) -> SyncDb {
        SyncDb {
            name: name.to_string(),
            packages: pkgs
                .iter()
                .map(|p| DbPackage {
                    name: p.to_string(),
                    version: "1.0-1".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn plain_install_is_kernel_then_headers() {
        let kernels =
            discover_kernels(&[db("cachyos", &["linux-cachyos", "linux-cachyos-headers"])]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile::default();
        let by_raw = kernels_by_raw(&kernels);
        let plan = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        assert_eq!(plan.install.len(), 2);
        assert_eq!(plan.install[0].package, "linux-cachyos");
        assert_eq!(plan.install[0].reason, Reason::SelectedKernel);
        assert_eq!(plan.install[1].package, "linux-cachyos-headers");
        assert_eq!(plan.install[1].reason, Reason::RequiredHeaders);
    }

    #[test]
    fn zfs_root_adds_zfs_companion_first() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile {
            root_on_zfs: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert_eq!(plan.install[0].package, "linux-cachyos-zfs");
        assert_eq!(plan.install[0].reason, Reason::ZfsRootCompanion);
        assert_eq!(plan.install[1].package, "linux-cachyos");
    }

    #[test]
    fn zfs_only_when_root_on_zfs() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;
        let hw = HardwareProfile {
            root_on_zfs: false,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-zfs"));
    }

    #[test]
    fn nvidia_matrix_chwd_and_dkms() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        // chwd says nvidia-dkms, no dkms installed -> prebuilt nvidia added
        let hw = HardwareProfile {
            chwd_nvidia: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia" && a.reason == Reason::NvidiaCompanion));

        // dkms installed -> no prebuilt
        let hw = HardwareProfile {
            chwd_nvidia: true,
            installed: ["nvidia-dkms".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));

        // no chwd -> nothing
        let plan = TransactionPlan::from_selection(
            &sel,
            &HardwareProfile::default(),
            &kernels_by_raw(&kernels),
        );
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));
    }

    #[test]
    fn nvidia_module_family_precedence() {
        // prebuilt nvidia installed -> prefer it even without chwd
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
                "linux-cachyos-nvidia-open",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;

        let hw = HardwareProfile {
            nvidia_modules_installed: true,
            chwd_nvidia_open: true, // would suggest open, but closed wins
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia"));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia-open"));

        let hw = HardwareProfile {
            nvidia_open_modules_installed: true,
            chwd_nvidia: true,
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        assert!(plan
            .install
            .iter()
            .any(|a| a.package == "linux-cachyos-nvidia-open"));
        assert!(plan
            .install
            .iter()
            .all(|a| a.package != "linux-cachyos-nvidia"));
    }

    #[test]
    fn removal_includes_only_installed_companions() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        // kernel + headers + zfs installed; installed-db == repo -> the row
        // is immutable and checked by default
        let mut local = std::collections::BTreeMap::new();
        local.insert(
            "linux-cachyos".to_string(),
            ("cachyos".to_string(), "6.14.1-1".to_string()),
        );
        sel.rows = to_rows(&kernels, &local);
        assert!(sel.rows[0].installed);
        assert!(sel.rows[0].immutable);
        assert!(sel.rows[0].checked);
        // user unchecks -> removal
        sel.rows[0].checked = false;

        // headers and zfs installed -> both removed
        let hw = HardwareProfile {
            installed: ["linux-cachyos-headers".into(), "linux-cachyos-zfs".into()]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        let pkgs: Vec<&str> = plan.remove.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(
            pkgs,
            vec![
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-zfs"
            ]
        );

        // zfs not installed -> not removed
        let hw = HardwareProfile {
            installed: ["linux-cachyos-headers".into()].into_iter().collect(),
            ..Default::default()
        };
        let plan = TransactionPlan::from_selection(&sel, &hw, &kernels_by_raw(&kernels));
        let pkgs: Vec<&str> = plan.remove.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(pkgs, vec!["linux-cachyos", "linux-cachyos-headers"]);
    }

    #[test]
    fn plan_is_deterministic() {
        let kernels = discover_kernels(&[db(
            "cachyos",
            &[
                "linux-cachyos",
                "linux-cachyos-headers",
                "linux-cachyos-nvidia",
                "linux-cachyos-zfs",
            ],
        )]);
        let mut sel = SelectionState::default();
        sel.rows = to_rows(&kernels, &Default::default());
        sel.rows[0].checked = true;
        let hw = HardwareProfile {
            root_on_zfs: true,
            chwd_nvidia: true,
            ..Default::default()
        };
        let by_raw = kernels_by_raw(&kernels);
        let p1 = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        let p2 = TransactionPlan::from_selection(&sel, &hw, &by_raw);
        assert_eq!(p1, p2);
        // install order: zfs, nvidia, kernel, headers
        let order: Vec<&str> = p1.install.iter().map(|a| a.package.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "linux-cachyos-zfs",
                "linux-cachyos-nvidia",
                "linux-cachyos",
                "linux-cachyos-headers"
            ]
        );
    }
}
