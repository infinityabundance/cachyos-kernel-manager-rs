//! Oracle lock: the immutable freeze record (`oracle/UPSTREAM.lock`),
//! archive verification, and upstream-revision diffing (directive §86).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// The frozen oracle authority record (parses `oracle/UPSTREAM.lock`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpstreamLock {
    pub oracle: OracleSection,
    pub identity: IdentitySurface,
    /// Free-form archaeology notes (e.g. `[quirks.inventory]`); informational.
    #[serde(default)]
    pub quirks: Option<toml::Value>,
}

/// The `[oracle]` section of the lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSection {
    pub repository: String,
    pub branch: String,
    pub commit: String,
    pub tree: String,
    pub version: String,
    #[serde(default)]
    pub tag: Option<String>,
    pub retrieved_at: String,
    #[serde(default)]
    pub source_archive: String,
    #[serde(default)]
    pub source_archive_hash: String,
    #[serde(default)]
    pub package_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub reference_image_hash: String,
}

/// Externally visible identity surfaces (directive §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySurface {
    pub binary: String,
    pub desktop_file: String,
    pub icon_id: String,
    pub polkit_action: String,
    pub polkit_policy_file: String,
    pub polkit_exec_path: String,
    pub polkit_defaults: BTreeMap<String, String>,
    pub helper_dir: String,
}

impl UpstreamLock {
    /// Parse the lock file (TOML).
    pub fn parse(content: &str) -> Result<UpstreamLock, OracleError> {
        Ok(toml::from_str(content)?)
    }

    /// Load the lock file from disk.
    pub fn load(path: &Path) -> Result<UpstreamLock, OracleError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Verify the source archive hash against `source_archive` (relative to
    /// the repository root). The lock may record the hash with a `sha256:`
    /// scheme prefix; it is normalized before comparison.
    pub fn verify_archive(&self, repo_root: &Path) -> Result<bool, OracleError> {
        let archive = repo_root.join(&self.oracle.source_archive);
        let bytes = std::fs::read(&archive)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        let expected = self
            .oracle
            .source_archive_hash
            .strip_prefix("sha256:")
            .unwrap_or(&self.oracle.source_archive_hash);
        Ok(actual == expected)
    }

    /// The git tree hash of `commit` as recorded (informational; verify with
    /// `git rev-parse` in xtask).
    pub fn tree(&self) -> &str {
        &self.oracle.tree
    }
}

/// Diff the frozen oracle revision against a candidate ref in a git clone.
/// Returns changed file names (`git diff --name-only old new`).
pub fn diff_revisions(
    git_dir: &Path,
    old_commit: &str,
    new_commit: &str,
) -> Result<Vec<String>, OracleError> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .arg("diff")
        .arg("--name-only")
        .arg(old_commit)
        .arg(new_commit)
        .output()?;
    if !out.status.success() {
        return Err(OracleError::Git(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("git error: {0}")]
    Git(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_parses_from_repo_file() {
        // The real lock must parse and carry the frozen authority facts.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/UPSTREAM.lock");
        let lock = UpstreamLock::load(&root).unwrap();
        assert_eq!(
            lock.oracle.repository,
            "https://github.com/CachyOS/kernel-manager"
        );
        assert_eq!(lock.oracle.branch, "develop");
        assert_eq!(
            lock.oracle.commit,
            "6b4a373e6a4e7295a0803034e597c4f2a055a411"
        );
        assert_eq!(lock.oracle.version, "1.19.0");
        assert_eq!(lock.identity.binary, "cachyos-kernel-manager");
        assert_eq!(
            lock.identity.polkit_action,
            "org.cachyos.KernelManager.pkexec.policy.run-root-terminal"
        );
        assert_eq!(
            lock.identity.polkit_exec_path,
            "/usr/lib/cachyos-kernel-manager/rootshell.sh"
        );
    }
}
