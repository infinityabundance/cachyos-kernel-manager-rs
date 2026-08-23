//! Court case directories: manifests (`claim.toml`, `assumptions.toml`,
//! `comparator.toml`), fingerprinting, comparison, and the residual ledger.
//!
//! Case layout (directive §44):
//! ```text
//! courts/<domain>/<case>/
//!   claim.toml  assumptions.toml  comparator.toml
//!   fixture/  oracle/  candidate/
//!   residual.json  evidence.json  README.md
//! ```

#![forbid(unsafe_code)]

pub mod evidence;
pub mod evidence_release;
pub mod normalize;
pub mod vm_court;

use crate::vm_court::CompanionExpectation;
use cachyos_kernel_manager_frf::{EvidentiaryChain, Residual};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `claim.toml` — the evidentiary chain for the court.
pub type Claim = EvidentiaryChain;

/// `assumptions.toml` — environmental assumptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assumptions {
    pub assumptions: Vec<String>,
    /// Normalizers applied to raw evidence (name -> version).
    pub normalizers: Vec<Normalizer>,
}

/// A declared normalizer (directive §46): every normalizer must be explicit
/// and versioned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Normalizer {
    pub name: String,
    pub version: String,
    /// sha256 of the normalizer source at run time.
    pub source_hash: String,
    pub input_domain: String,
    pub transformation: String,
    pub justification: String,
    pub falsifier: String,
}

/// `comparator.toml` — how oracle and candidate observations are compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comparator {
    pub version: String,
    /// Fixture image to bake for this court (`vm/fixtures/<fixture>`);
    /// defaults to the case name.
    #[serde(default)]
    pub fixture: Option<String>,
    /// Paths (relative to oracle/ or candidate/) excluded from comparison.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Paths compared byte-for-byte.
    #[serde(default)]
    pub byte_exact: Vec<String>,
    /// Paths compared as JSON semantics (parse + canonical compare).
    #[serde(default)]
    pub json_semantic: Vec<String>,
    /// Volatile path patterns normalized before comparison (e.g. PIDs).
    #[serde(default)]
    pub volatile_prefixes: Vec<String>,
    /// Source-anchored companion expectations, keyed by kernel name
    /// (`kernel.cpp:226-244`). When present, the candidate's per-row
    /// companions are compared against this model and mismatches become
    /// residuals.
    #[serde(default)]
    pub companion_model: BTreeMap<String, CompanionExpectation>,
    /// Phase 5 transaction courts: when present, the runner drives a real
    /// GUI transaction on the oracle side (AT-SPI checkbox toggle + Execute)
    /// under strace and runs the candidate plan tool, comparing the
    /// witnessed exec chains (probes, pacman argv, terminal-helper argv).
    #[serde(default)]
    pub transaction: Option<TransactionSpec>,
    /// Phase 5 terminal-matrix court: when present, the runner executes the
    /// terminal-helper script (oracle: the frozen upstream script; candidate:
    /// the packaged copy) against the emulator-stub fixture scenarios and
    /// compares exit codes + outputs.
    #[serde(default)]
    pub terminal_matrix: Option<serde_json::Value>,
    /// Phase 6 configure-flow court (git-cache/lifecycle): when true, the
    /// runner drives the real GUI Configure button (AT-SPI) under strace on
    /// the oracle side and runs the candidate's git-cache model, comparing
    /// the witnessed git exec chain (prepare_git_repo argv).
    #[serde(default)]
    pub configure: bool,
    /// Phase 6 mutation court (patch-injection/*, custom-name/*): when
    /// present, the runner drives the real Configure window (sets the custom
    /// name, adds a remote patch, clicks Build kernel) on the oracle side and
    /// runs the candidate's mutation model against the same fixture PKGBUILD,
    /// comparing the mutated PKGBUILD byte-for-byte.
    #[serde(default)]
    pub mutate: Option<MutationSpec>,
    /// Phase 7 scx court (scx/loader-interface --vm): when true, the runner
    /// starts the REAL scx_loader on the system bus in the VM (oracle side)
    /// and runs the candidate's typed client against the same bus (candidate
    /// side), comparing the candidate's declared interface as a SUBSET of the
    /// real loader's interface + the property readback values.
    #[serde(default)]
    pub scx: bool,
    /// Phase 9 gap-006 court (build-env/makepkg-runtime --vm): when true,
    /// the runner executes the oracle's literal build commands (oracle
    /// side) and the candidate's MODEL-rendered build commands (candidate
    /// side) under strace, comparing the extracted execve chains.
    #[serde(default)]
    pub makepkg: bool,
    /// Phase 10 packaging court (packaging/upgrade --vm): when true, the
    /// runner drives the oracle->candidate->oracle package transition on
    /// both sides (identical scripts) and compares the surfaces.
    #[serde(default)]
    pub packaging: bool,
    /// Phase 11 boot court (boot/system-boot-after-install --vm): when
    /// true, the runner runs the install phase, REBOOTS the same overlay,
    /// and runs the boot-check phase.
    #[serde(default)]
    pub boot: bool,
    /// Phase 11 removal court (boot/system-boot-after-remove --vm): like
    /// `boot`, but the phase-1 scripts SET UP the two-kernel state and
    /// REMOVE the second kernel; the reboot check asserts the removal
    /// persisted.
    #[serde(default)]
    pub boot_remove: bool,
    /// Phase 11 failed-boot court (boot/system-boot-after-failure --vm):
    /// like `boot_remove`, but the phase-1 scripts REMOVE the RUNNING
    /// kernel (the base, which the qemu direct boot loads); the reboot
    /// check asserts the FAILED state (the running kernel's packages + its
    /// /boot entry are GONE — a real machine would fail its next boot from
    /// its own disk — while the harness's direct-kernel boot still brings
    /// the machine up for the residual witness).
    #[serde(default)]
    pub boot_failure: bool,
    /// Phase 11 multi-reboot drift court (boot/system-boot-drift --vm):
    /// the install mutation, then the SAME overlay reboots N times with a
    /// suffixed boot-check after each; every reboot surface must be
    /// byte-identical across boots AND sides (no drift).
    #[serde(default)]
    pub boot_drift: bool,
    /// Phase 12 production-integration slice (ui/gui-drive --vm): when
    /// true, the runner drives the PACKAGED GUI (oracle side: the frozen Qt
    /// binary; candidate side: the release binary staged into the share)
    /// through the sort → stable-identity → toggle workflow under
    /// Xvfb + AT-SPI (candidate-drive.py is side-agnostic) and compares the
    /// SEMANTIC sequence (the sorted pkgname order per header + the toggle
    /// identity proof) byte-for-byte + the machine residual.
    #[serde(default)]
    pub gui_drive: bool,
    /// Phase 12 hostile-review rendered-i18n court (ui/i18n-rendered --vm):
    /// when true, the runner drives the PACKAGED GUI under a GENERATED
    /// non-English locale (de_DE.UTF-8 + zh_CN.UTF-8) and compares the
    /// RENDERED main-window accessible projection (window title, the
    /// description, the four tree headers, the action buttons) byte-for-byte
    /// — the audit P2 requirement that the i18n courts witness rendered
    /// production projections, not just catalog lookup (and gap-009's
    /// rendered zh_CN projection: BOTH sides show English — the oracle
    /// never loads its CJK catalog because QLocale reports zh_CN vs the
    /// zh-CN qrc alias).
    #[serde(default)]
    pub i18n_rendered: bool,
    /// Phase 12 hostile-review gap-010 court (ui/close-during-transaction
    /// --vm): when true, the runner drives each side's PACKAGED GUI through
    /// a REAL transaction (toggle + Execute), closes the MAIN window while
    /// the transaction is in-flight (WM_DELETE_WINDOW), and compares the
    /// machine residuals byte-for-byte + validates the documented D-008
    /// exit-outcome divergence (the oracle ABORTS — Qt's QThread destroyed
    /// while running after closeEvent's alpm_release — the candidate exits
    /// CLEANLY).
    #[serde(default)]
    pub close_transaction: bool,
}

/// The `[mutate]` comparator section: the Configure-window actions the
/// runner performs on the oracle side and feeds to the candidate model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MutationSpec {
    /// The custom-name text to set (empty = leave the window default
    /// `$pkgbase-custom`).
    pub custom_name: String,
    /// The remote patch URL to add (empty = add none).
    pub patch_url: String,
}

/// The `[transaction]` comparator section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransactionSpec {
    /// The tree rows (raw `<repo>/<kernel>`) the AT-SPI driver toggles
    /// (checkbox flipped from its default state).
    #[serde(default)]
    pub select: Vec<String>,
}

/// A loaded case directory.
#[derive(Debug)]
pub struct Case {
    pub domain: String,
    pub name: String,
    pub dir: PathBuf,
    pub claim: Claim,
    pub assumptions: Assumptions,
    pub comparator: Comparator,
}

impl Case {
    /// Load a case from `courts/<domain>/<case>/`.
    pub fn load(domain: &str, name: &str, root: &Path) -> Result<Case, CaseError> {
        let dir = root.join(domain).join(name);
        let claim = parse_toml(&dir.join("claim.toml"))?;
        let assumptions = parse_toml(&dir.join("assumptions.toml"))?;
        let comparator = parse_toml(&dir.join("comparator.toml"))?;
        Ok(Case {
            domain: domain.into(),
            name: name.into(),
            dir,
            claim,
            assumptions,
            comparator,
        })
    }

    /// Fingerprint a directory (e.g. `oracle/`, `candidate/`, `fixture/`):
    /// relpath -> sha256. Missing directory -> empty map (court not run).
    pub fn fingerprint(&self, sub: &str) -> Result<BTreeMap<String, String>, CaseError> {
        let base = self.dir.join(sub);
        if !base.exists() {
            return Ok(BTreeMap::new());
        }
        fingerprint_tree(&base)
    }

    /// Compare oracle/ vs candidate/ per the comparator rules.
    pub fn compare(&self) -> Result<Vec<Residual>, CaseError> {
        let oracle = self.fingerprint("oracle")?;
        let candidate = self.fingerprint("candidate")?;
        let mut residuals = Vec::new();
        let mut paths: Vec<&String> = oracle.keys().chain(candidate.keys()).collect();
        paths.sort();
        paths.dedup();
        for path in paths {
            if self
                .comparator
                .ignore
                .iter()
                .any(|i| path == i || path.starts_with(&format!("{i}/")))
            {
                continue;
            }
            match (oracle.get(path), candidate.get(path)) {
                (Some(a), Some(c)) => {
                    if a != c {
                        residuals.push(Residual {
                            id: format!("{}-{}", self.name, path.replace('/', "-")),
                            court: format!("{}/{}", self.domain, self.name),
                            layer: "filesystem".into(),
                            oracle_fingerprint: a.clone(),
                            candidate_fingerprint: c.clone(),
                            classification: "deterministic_mismatch".into(),
                            root_cause: None,
                            resolution: None,
                            commit: None,
                            regression_witness: None,
                        });
                    }
                }
                (Some(a), None) => residuals.push(Residual {
                    id: format!("{}-{}", self.name, path.replace('/', "-")),
                    court: format!("{}/{}", self.domain, self.name),
                    layer: "filesystem".into(),
                    oracle_fingerprint: a.clone(),
                    candidate_fingerprint: "<absent>".into(),
                    classification: "missing_candidate_artifact".into(),
                    root_cause: None,
                    resolution: None,
                    commit: None,
                    regression_witness: None,
                }),
                (None, Some(c)) => residuals.push(Residual {
                    id: format!("{}-{}", self.name, path.replace('/', "-")),
                    court: format!("{}/{}", self.domain, self.name),
                    layer: "filesystem".into(),
                    oracle_fingerprint: "<absent>".into(),
                    candidate_fingerprint: c.clone(),
                    classification: "missing_oracle_artifact".into(),
                    root_cause: None,
                    resolution: None,
                    commit: None,
                    regression_witness: None,
                }),
                (None, None) => {}
            }
        }
        Ok(residuals)
    }
}

/// Hash every file under `base`, relative path -> sha256 hex.
pub fn fingerprint_tree(base: &Path) -> Result<BTreeMap<String, String>, CaseError> {
    let mut out = BTreeMap::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(base) {
                let rel = rel.to_string_lossy().to_string();
                out.insert(rel, sha256_file(&path)?);
            }
        }
    }
    Ok(out)
}

/// sha256 of a file.
pub fn sha256_file(path: &Path) -> Result<String, CaseError> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// sha256 of a byte string.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Errors from case handling.
#[derive(Debug, thiserror::Error)]
pub enum CaseError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CaseError> {
    let content = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&content)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_are_stable_and_content_addressed() {
        let dir = std::env::temp_dir().join(format!("km-casefile-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        std::fs::write(dir.join("sub/b.txt"), b"world").unwrap();
        let fp = fingerprint_tree(&dir).unwrap();
        assert_eq!(fp.get("a.txt").unwrap(), &sha256_bytes(b"hello"));
        assert_eq!(fp.get("sub/b.txt").unwrap(), &sha256_bytes(b"world"));
        // deterministic
        let fp2 = fingerprint_tree(&dir).unwrap();
        assert_eq!(fp, fp2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
