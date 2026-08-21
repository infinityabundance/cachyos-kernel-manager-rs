//! Content-addressed evidence records (directive §77, §44).

#![forbid(unsafe_code)]

use crate::sha256_file;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One artifact reference with its content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceArtifact {
    /// Repository-relative or case-relative path.
    pub path: String,
    /// sha256 hex.
    pub sha256: String,
}

/// The evidence record for one court run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub court: String,
    pub oracle_revision: String,
    pub candidate_revision: String,
    pub fixture_digest: Option<String>,
    /// Normalizer versions used (name -> version).
    pub normalizers: Vec<(String, String)>,
    pub comparator_version: String,
    /// Result: "pass" | "fail" | "pending".
    pub result: String,
    /// Residual count after comparison.
    pub residual_count: usize,
    /// All artifacts, content-addressed.
    pub artifacts: Vec<EvidenceArtifact>,
}

impl EvidenceRecord {
    /// Hash every file under `dir` into `artifacts` (relative paths).
    pub fn add_directory(&mut self, dir: &Path, prefix: &str) -> Result<(), crate::CaseError> {
        for (rel, hash) in crate::fingerprint_tree(dir)? {
            self.artifacts.push(EvidenceArtifact {
                path: format!("{prefix}/{rel}"),
                sha256: hash,
            });
        }
        Ok(())
    }

    /// Add one file as an artifact under `prefix/` (its base name).
    pub fn add_file(&mut self, file: &Path, prefix: &str) -> Result<(), crate::CaseError> {
        let name = file
            .file_name()
            .ok_or_else(|| crate::CaseError::Other(format!("no file name: {}", file.display())))?
            .to_string_lossy()
            .into_owned();
        let hash = crate::sha256_file(file)?;
        self.artifacts.push(EvidenceArtifact {
            path: format!("{prefix}/{name}"),
            sha256: hash,
        });
        Ok(())
    }

    /// Write `evidence.json` into `case_dir`.
    pub fn write(&self, case_dir: &Path) -> Result<(), crate::CaseError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(case_dir.join("evidence.json"), json)?;
        Ok(())
    }

    /// Load `evidence.json`.
    pub fn load(case_dir: &Path) -> Result<EvidenceRecord, crate::CaseError> {
        let content = std::fs::read_to_string(case_dir.join("evidence.json"))?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Verify every artifact hash against the filesystem. Returns paths
    /// whose hash does not match (empty = all verified).
    pub fn verify(&self, base: &Path) -> Result<Vec<String>, crate::CaseError> {
        let mut bad = Vec::new();
        for artifact in &self.artifacts {
            let path = base.join(&artifact.path);
            if !path.exists() {
                bad.push(format!("{} (missing)", artifact.path));
                continue;
            }
            let actual = sha256_file(&path)?;
            if actual != artifact.sha256 {
                bad.push(format!("{} (hash mismatch)", artifact.path));
            }
        }
        Ok(bad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_round_trip_and_verify() {
        let dir = std::env::temp_dir().join(format!("km-evidence-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        std::fs::write(dir.join("out/a.txt"), b"hello").unwrap();

        let mut ev = EvidenceRecord {
            court: "kernel-discovery/minimal".into(),
            oracle_revision: "x".into(),
            candidate_revision: "y".into(),
            fixture_digest: None,
            normalizers: vec![("a11y".into(), "1.0.0".into())],
            comparator_version: "1.0.0".into(),
            result: "pass".into(),
            residual_count: 0,
            artifacts: vec![],
        };
        ev.add_directory(&dir.join("out"), "out").unwrap();
        ev.write(&dir).unwrap();
        let loaded = EvidenceRecord::load(&dir).unwrap();
        assert_eq!(loaded, ev);
        assert!(loaded.verify(&dir).unwrap().is_empty());

        // tamper detection
        std::fs::write(dir.join("out/a.txt"), b"tampered").unwrap();
        assert_eq!(loaded.verify(&dir).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
