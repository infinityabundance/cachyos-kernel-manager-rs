//! Evidence releases (directive §89 — the publication layer).
//!
//! Two concepts, deliberately distinct:
//!
//! - **court recipe** — the committed, reproducible instructions: claim/
//!   assumptions/comparator/README + frozen fixture inputs. How to reproduce
//!   evidence.
//! - **evidence release** — an immutable, content-addressed record of an
//!   ACTUAL execution: per-court artifact hashes (oracle/candidate/residual/
//!   evidence), normalizer + comparator versions, fixture + image digests,
//!   and a root hash over the whole release. The raw (often huge) artifacts
//!   stay outside the repository (gitignored, regenerable, hash-verifiable);
//!   the release files — `MANIFEST.json`, `COURTS.json`, `RECEIPTS.json`,
//!   `ROOT-HASH` — are small and are committed.
//!
//! Layout:
//! ```text
//! evidence/releases/<name>/
//!   MANIFEST.json   release metadata + root hash + counts
//!   COURTS.json     per-court full receipts (recipe + evidence + locator)
//!   RECEIPTS.json   compact FRF receipts (claim -> evidence -> receipt hash)
//!   ROOT-HASH       sha256 of the canonical COURTS.json bytes
//! ```

#![forbid(unsafe_code)]

use crate::sha256_bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the raw evidence lives. Today the raw artifacts are regenerable
/// from the recipe (the runner re-executes the court); the hashes make any
/// future archive (GitHub Release asset, Zenodo, OCI artifact) verifiable
/// against this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locator {
    /// Raw evidence regenerable from the recipe + committed inputs; the
    /// listed paths are the court-relative artifact locations.
    Regenerable { paths: Vec<String> },
    /// Raw evidence archived externally (content-addressed URL/identifier).
    Archived { uri: String },
}

/// Hashes of the committed recipe files (how to reproduce the evidence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeRef {
    pub claim_sha256: String,
    pub assumptions_sha256: String,
    pub comparator_sha256: String,
    pub readme_sha256: String,
    /// Tree hash of the committed `fixture/` inputs (corpus, static files);
    /// `None` when the court has none.
    pub fixture_tree_sha256: Option<String>,
}

/// One court's full receipt inside a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourtReceipt {
    pub court: String,
    pub recipe: RecipeRef,
    pub result: String,
    pub residual_count: usize,
    pub normalizers: Vec<(String, String)>,
    pub comparator_version: String,
    pub oracle_revision: String,
    pub candidate_revision: String,
    pub fixture_digest: Option<String>,
    /// Content-addressed raw evidence (court-relative paths).
    pub artifacts: BTreeMap<String, String>,
    pub locator: Locator,
    /// sha256 of this receipt's canonical JSON — the FRF receipt hash.
    pub receipt_sha256: String,
}

/// The release manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub release: String,
    pub created_at: String,
    pub git_commit: String,
    pub oracle_revision: String,
    pub candidate_revision: String,
    pub base_image_hash: String,
    pub root_hash: String,
    pub courts: usize,
    pub pass: usize,
    pub fail: usize,
    pub unrecorded: usize,
    pub schema: String,
}

/// A compact FRF receipt (claim -> evidence -> receipt hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrfReceipt {
    pub court: String,
    pub claim_sha256: String,
    pub receipt_sha256: String,
    pub result: String,
    pub artifact_count: usize,
}

impl CourtReceipt {
    /// Recompute the receipt hash from the canonical bytes of `self`
    /// (deterministic serde_json::to_vec).
    pub fn recompute_receipt_hash(&mut self) {
        self.receipt_sha256 = String::new();
        let bytes = serde_json::to_vec(self).expect("serialize receipt");
        self.receipt_sha256 = sha256_bytes(&bytes);
    }
}

/// Build and verify evidence releases over `courts/`.
pub struct ReleaseBuilder {
    pub release: String,
    pub created_at: String,
    pub git_commit: String,
    pub oracle_revision: String,
    pub candidate_revision: String,
    pub base_image_hash: String,
}

impl ReleaseBuilder {
    /// Collect receipts for every court under `courts_root` (a dir of
    /// domain/case dirs each with `claim.toml`).
    pub fn collect(&self, courts_root: &Path) -> Vec<CourtReceipt> {
        let mut receipts = Vec::new();
        let mut domains: Vec<_> = std::fs::read_dir(courts_root)
            .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default();
        domains.sort_by_key(|d| d.file_name());
        for domain in domains {
            if !domain.path().is_dir() {
                continue;
            }
            let mut cases: Vec<_> = std::fs::read_dir(domain.path())
                .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
                .unwrap_or_default();
            cases.sort_by_key(|c| c.file_name());
            for case in cases {
                let dir = case.path();
                if !dir.join("claim.toml").exists() {
                    continue;
                }
                let court = format!(
                    "{}/{}",
                    domain.file_name().to_string_lossy(),
                    case.file_name().to_string_lossy()
                );
                if let Some(r) = self.receipt(&court, &dir) {
                    receipts.push(r);
                }
            }
        }
        receipts
    }

    fn receipt(&self, court: &str, dir: &Path) -> Option<CourtReceipt> {
        let hash = |p: &Path| crate::sha256_file(p).ok();
        let recipe = RecipeRef {
            claim_sha256: hash(&dir.join("claim.toml"))?,
            assumptions_sha256: hash(&dir.join("assumptions.toml"))?,
            comparator_sha256: hash(&dir.join("comparator.toml"))?,
            readme_sha256: hash(&dir.join("README.md")).unwrap_or_default(),
            fixture_tree_sha256: tree_hash(&dir.join("fixture")),
        };

        // evidence.json (the VM runner's record) when present
        let (result, residual_count, normalizers, comparator_version, fixture_digest) =
            match crate::evidence::EvidenceRecord::load(dir) {
                Ok(ev) => (
                    ev.result,
                    ev.residual_count,
                    ev.normalizers,
                    ev.comparator_version,
                    ev.fixture_digest,
                ),
                Err(_) => ("unrecorded".to_string(), 0, Vec::new(), String::new(), None),
            };

        // content-addressed artifacts from the filesystem (oracle/,
        // candidate/, residual.json, evidence.json)
        let mut artifacts = BTreeMap::new();
        for sub in ["oracle", "candidate"] {
            if let Ok(files) = crate::fingerprint_tree(&dir.join(sub)) {
                for (rel, h) in files {
                    artifacts.insert(format!("{sub}/{rel}"), h);
                }
            }
        }
        for file in ["residual.json", "evidence.json"] {
            if let Ok(h) = crate::sha256_file(&dir.join(file)) {
                artifacts.insert(file.to_string(), h);
            }
        }

        let locator = Locator::Regenerable {
            paths: vec![
                "oracle/".into(),
                "candidate/".into(),
                "residual.json".into(),
                "evidence.json".into(),
            ],
        };

        let mut receipt = CourtReceipt {
            court: court.to_string(),
            recipe,
            result,
            residual_count,
            normalizers,
            comparator_version,
            oracle_revision: self.oracle_revision.clone(),
            candidate_revision: self.candidate_revision.clone(),
            fixture_digest,
            artifacts,
            locator,
            receipt_sha256: String::new(),
        };
        receipt.recompute_receipt_hash();
        Some(receipt)
    }

    /// Assemble the full release: COURTS.json (canonical), RECEIPTS.json,
    /// MANIFEST.json, ROOT-HASH. Returns the four file contents.
    pub fn assemble(&self, courts_root: &Path) -> (String, String, String, String) {
        let receipts = self.collect(courts_root);
        let courts_json = serde_json::to_string_pretty(&receipts).expect("serialize courts");
        let root_hash = sha256_bytes(courts_json.as_bytes());

        let frf: Vec<FrfReceipt> = receipts
            .iter()
            .map(|r| FrfReceipt {
                court: r.court.clone(),
                claim_sha256: r.recipe.claim_sha256.clone(),
                receipt_sha256: r.receipt_sha256.clone(),
                result: r.result.clone(),
                artifact_count: r.artifacts.len(),
            })
            .collect();
        let receipts_json = serde_json::to_string_pretty(&frf).expect("serialize receipts");

        let pass = receipts.iter().filter(|r| r.result == "pass").count();
        let fail = receipts.iter().filter(|r| r.result == "fail").count();
        let unrecorded = receipts.iter().filter(|r| r.result == "unrecorded").count();
        let manifest = ReleaseManifest {
            release: self.release.clone(),
            created_at: self.created_at.clone(),
            git_commit: self.git_commit.clone(),
            oracle_revision: self.oracle_revision.clone(),
            candidate_revision: self.candidate_revision.clone(),
            base_image_hash: self.base_image_hash.clone(),
            root_hash: root_hash.clone(),
            courts: receipts.len(),
            pass,
            fail,
            unrecorded,
            schema: "cachyos-km-evidence-release-v1".into(),
        };
        let manifest_json = serde_json::to_string_pretty(&manifest).expect("serialize manifest");

        (manifest_json, courts_json, receipts_json, root_hash)
    }

    /// Write the release into `evidence/releases/<name>/`.
    pub fn write_release(&self, repo_root: &Path, courts_root: &Path) -> Result<PathBuf, String> {
        let (manifest, courts, receipts, root) = self.assemble(courts_root);
        let dir = repo_root.join("evidence/releases").join(&self.release);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("MANIFEST.json"), manifest).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("COURTS.json"), courts).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("RECEIPTS.json"), receipts).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("ROOT-HASH"), format!("{root}\n")).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}

/// Deterministic tree hash of a directory (sorted relative paths ->
/// sha256 bytes; empty dir -> None).
pub fn tree_hash(dir: &Path) -> Option<String> {
    let files = crate::fingerprint_tree(dir).ok()?;
    if files.is_empty() {
        return None;
    }
    let mut buf = Vec::new();
    for (rel, hash) in files {
        buf.extend_from_slice(rel.as_bytes());
        buf.push(0);
        buf.extend_from_slice(hash.as_bytes());
        buf.push(0);
    }
    Some(sha256_bytes(&buf))
}

/// Verify a written release against the current filesystem: every court's
/// recorded artifact hashes (recomputed from oracle//candidate//residual/
/// evidence when present), the receipt hashes, and the root hash. Returns
/// a list of problems (empty = verified).
pub fn verify_release(
    repo_root: &Path,
    courts_root: &Path,
    release: &str,
) -> Result<Vec<String>, String> {
    let dir = repo_root.join("evidence/releases").join(release);
    let mut problems = Vec::new();

    let courts: Vec<CourtReceipt> = serde_json::from_str(
        &std::fs::read_to_string(dir.join("COURTS.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let mut domains: Vec<_> = std::fs::read_dir(courts_root)
        .map(|it| it.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default();
    domains.sort_by_key(|d| d.file_name());

    for receipt in &courts {
        let Some((domain, name)) = receipt.court.split_once('/') else {
            problems.push(format!("{}: bad court id", receipt.court));
            continue;
        };
        let dir = courts_root.join(domain).join(name);
        for (rel, recorded) in &receipt.artifacts {
            let path = dir.join(rel);
            match crate::sha256_file(&path) {
                Ok(actual) if &actual == recorded => {}
                Ok(actual) => problems.push(format!(
                    "{}: {} hash mismatch (recorded {} actual {})",
                    receipt.court, rel, recorded, actual
                )),
                Err(_) => problems.push(format!(
                    "{}: {} missing (raw evidence not present locally — expected for a fresh clone; hash is on record)",
                    receipt.court, rel
                )),
            }
        }
    }

    // recompute the root hash from the current COURTS.json and compare
    let courts_now = std::fs::read(dir.join("COURTS.json")).map_err(|e| e.to_string())?;
    let root_now = sha256_bytes(&courts_now);
    let root_recorded = std::fs::read_to_string(dir.join("ROOT-HASH"))
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    if root_now != root_recorded {
        problems.push(format!(
            "ROOT-HASH mismatch: recorded {root_recorded} recomputed {root_now}"
        ));
    }
    Ok(problems)
}
