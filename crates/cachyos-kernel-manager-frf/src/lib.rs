//! FRF (Forensic Residual Framework) evidentiary-chain and receipt types.
//!
//! Every court encodes the chain
//! `CLAIM → MODEL → ASSUMPTIONS → OBSERVABLES → WITNESS → INDEPENDENCE →
//! FALSIFIER → EVIDENCE` (directive §0). These types give the chain a
//! machine-readable form; the `casefile` crate turns them into reproducible
//! case directories, and `xtask` runs them.
//!
//! Receipts are never hand-written: they are produced by the runner and
//! bound to content hashes (docs/COURTS.md).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The eight-element evidentiary chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidentiaryChain {
    /// What precise compatibility claim is being made.
    pub claim: String,
    /// The state machine / semantic model representing expected behavior.
    pub model: String,
    /// Environmental assumptions making the comparison meaningful.
    pub assumptions: Vec<String>,
    /// Externally observable outputs or side effects that matter.
    pub observables: Vec<String>,
    /// The concrete execution that demonstrates the claim.
    pub witness: String,
    /// Why the witness does not merely reproduce the candidate's own
    /// assumptions.
    pub independence: String,
    /// The specific result that would prove the claim false.
    pub falsifier: String,
    /// Immutable artifacts proving what actually happened.
    pub evidence: Vec<EvidenceRef>,
}

/// A reference to an immutable evidence artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Repository-relative path to the artifact.
    pub artifact: String,
    /// Content hash (sha256).
    pub sha256: String,
}

/// Status of a court receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    /// All comparators passed; zero unexplained residuals.
    Passed,
    /// Comparator mismatch recorded in the residual ledger.
    Failed,
    /// Court defined but not yet executed.
    Pending,
    /// Execution produced nondeterminism (drift recorded).
    Flaky,
}

/// A court receipt, produced by the runner — never hand-written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// Court id, e.g. `kernel-discovery/ordinary`.
    pub court: String,
    /// Oracle revision the court was run against.
    pub oracle_revision: String,
    /// Candidate revision.
    pub candidate_revision: String,
    /// VM image digest, when the court runs in a VM.
    pub vm_image_digest: Option<String>,
    /// Fixture digest.
    pub fixture_digest: String,
    pub status: ReceiptStatus,
    /// Normalizer versions used (name -> version).
    pub normalizers: Vec<(String, String)>,
    /// Comparator version.
    pub comparator_version: String,
    /// Residuals (empty when parity holds).
    pub residuals: Vec<Residual>,
    /// Evidence artifacts, content-addressed.
    pub evidence: Vec<EvidenceRef>,
}

/// One recorded residual (directive §45). A mismatch is the most valuable
/// artifact in the project; it is never silently normalized away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Residual {
    pub id: String,
    pub court: String,
    pub layer: String,
    pub oracle_fingerprint: String,
    pub candidate_fingerprint: String,
    /// classification: deterministic_mismatch | oracle_nondeterminism |
    /// candidate_nondeterminism | environment_nondeterminism | historical |
    /// underspecified
    pub classification: String,
    pub root_cause: Option<String>,
    pub resolution: Option<String>,
    pub commit: Option<String>,
    pub regression_witness: Option<String>,
}

/// Discrepancy classification (directive §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscrepancyClass {
    RequiredParity,
    CompatibilityQuirk,
    KnownBugCompatibility,
    UnderspecifiedBehavior,
    IntentionalCorrection,
    SecurityCorrection,
}
