//! VM court comparison: align oracle and candidate observations, compare,
//! and produce the residual + evidence records (directive §44, §45, §46).

#![forbid(unsafe_code)]

use crate::normalize::{
    candidate_observation, oracle_observation, residual_digest, NormalizerError, Observation,
    A11Y_NORMALIZER_VERSION, CANDIDATE_NORMALIZER_VERSION, RESIDUAL_NORMALIZER_VERSION,
};
use crate::CaseError;
use cachyos_kernel_manager_frf::Residual;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Comparator version (bump on comparison-semantics changes).
pub const COMPARATOR_VERSION: &str = "1.1.0";

/// Source-anchored companion expectation from `comparator.toml`
/// (`[companion_model]`, keyed by kernel NAME). The oracle does not expose
/// companion resolution through AT-SPI at discovery time, so the candidate's
/// companions are compared against this model, which is derived from
/// `kernel.cpp:226-244` and recorded in the court claim. The differential
/// companion proof arrives with the Phase 5 transaction courts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompanionExpectation {
    #[serde(default)]
    pub zfs: Option<String>,
    #[serde(default)]
    pub nvidia: Option<String>,
    #[serde(default)]
    pub nvidia_open: Option<String>,
}

/// Read a JSON file.
fn read_json(path: &Path) -> Result<Value, CaseError> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Compare two observations field-by-field; returns residuals.
pub fn compare_observations(
    court: &str,
    oracle: &Observation,
    candidate: &Observation,
    companion_model: &BTreeMap<String, CompanionExpectation>,
) -> Vec<Residual> {
    let mut residuals = Vec::new();

    if oracle.rows.len() != candidate.rows.len() {
        residuals.push(Residual {
            id: format!("{court}-row-count"),
            court: court.into(),
            layer: "kernel-rows".into(),
            oracle_fingerprint: oracle.rows.len().to_string(),
            candidate_fingerprint: candidate.rows.len().to_string(),
            classification: "deterministic_mismatch".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    let n = oracle.rows.len().max(candidate.rows.len());
    for i in 0..n {
        let o = oracle.rows.get(i);
        let c = candidate.rows.get(i);
        match (o, c) {
            (Some(o), Some(c)) => {
                if o.raw != c.raw {
                    residuals.push(mismatch(court, i, "raw", &o.raw, &c.raw));
                }
                if o.version != c.version {
                    residuals.push(mismatch(court, i, "version", &o.version, &c.version));
                }
                if o.category != c.category {
                    residuals.push(mismatch(court, i, "category", &o.category, &c.category));
                }
                if o.checked != c.checked {
                    residuals.push(mismatch(
                        court,
                        i,
                        "checked",
                        &o.checked.to_string(),
                        &c.checked.to_string(),
                    ));
                }
                // companions: source-anchored model check (the oracle side
                // carries None; the model keys on the row's RAW `<repo>/<name>`
                // so per-repo duplicates can carry distinct expectations)
                if let Some(expected) = companion_model.get(&c.raw) {
                    if let Some(companions) = &c.companions {
                        for (field, actual, want) in [
                            ("zfs", &companions.zfs, &expected.zfs),
                            ("nvidia", &companions.nvidia, &expected.nvidia),
                            (
                                "nvidia_open",
                                &companions.nvidia_open,
                                &expected.nvidia_open,
                            ),
                        ] {
                            if actual != want {
                                residuals.push(mismatch(
                                    court,
                                    i,
                                    &format!("companion-{field}"),
                                    &want.clone().unwrap_or_default(),
                                    &actual.clone().unwrap_or_default(),
                                ));
                            }
                        }
                    }
                }
            }
            (Some(o), None) => residuals.push(mismatch(court, i, "row", &o.raw, "<missing>")),
            (None, Some(c)) => residuals.push(mismatch(court, i, "row", "<missing>", &c.raw)),
            (None, None) => {}
        }
    }

    // dialog semantics: the oracle shows the "No kernels found!" dialog
    // exactly when discovery is empty; the candidate has no GUI, so the
    // comparable observable is the CONDITION (rows empty).
    let oracle_empty_dialog = oracle
        .dialogs
        .iter()
        .any(|d| d.contains("No kernels found"));
    if oracle_empty_dialog != candidate.rows.is_empty() {
        residuals.push(mismatch(
            court,
            0,
            "empty-discovery-dialog-condition",
            &oracle_empty_dialog.to_string(),
            &candidate.rows.is_empty().to_string(),
        ));
    }

    residuals
}

fn mismatch(court: &str, row: usize, field: &str, oracle: &str, candidate: &str) -> Residual {
    Residual {
        id: format!("{court}-row{row}-{field}"),
        court: court.into(),
        layer: "kernel-rows".into(),
        oracle_fingerprint: oracle.to_string(),
        candidate_fingerprint: candidate.to_string(),
        classification: "deterministic_mismatch".into(),
        root_cause: None,
        resolution: None,
        commit: None,
        regression_witness: None,
    }
}

/// Run the full observation comparison for a court case directory:
/// `oracle/` and `candidate/` subdirectories each contain the observation
/// outputs (oracle-state.json / candidate-state.json / residual.json).
pub fn compare_vm_observations(
    case_dir: &Path,
    court_id: &str,
    companion_model: &BTreeMap<String, CompanionExpectation>,
) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    // 1. machine residuals must be byte-identical (fixture-integrity check):
    //    both runs happen on fresh overlays of the SAME fixture image.
    let oracle_residual = read_json(&oracle_dir.join("residual.json"))?;
    // NOTE: `or_else`, NOT `unwrap_or` — unwrap_or evaluates its argument
    // EAGERLY, so the fallback read would run (and fail on the renamed
    // file) even when the primary file exists.
    let candidate_residual = read_json(&candidate_dir.join("candidate-residual.json"))
        .or_else(|_| read_json(&candidate_dir.join("residual.json")))?;
    let o_digest = residual_digest(&oracle_residual).unwrap_or_else(|e| format!("ERR:{e}"));
    let c_digest = residual_digest(&candidate_residual).unwrap_or_else(|e| format!("ERR:{e}"));
    if o_digest != c_digest {
        residuals.push(Residual {
            id: format!("{court_id}-machine-residual-drift"),
            court: court_id.into(),
            layer: "machine-residual".into(),
            oracle_fingerprint: o_digest,
            candidate_fingerprint: c_digest,
            classification: "fixture_drift".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    // 2. kernel rows + dialogs
    let oracle_state = read_json(&oracle_dir.join("oracle-state.json"))?;
    let candidate_state = read_json(&candidate_dir.join("candidate-state.json"))?;
    let o_obs = oracle_observation(&oracle_state)
        .map_err(|e: NormalizerError| CaseError::Other(format!("oracle normalize: {e}")))?;
    let c_obs = candidate_observation(&candidate_state)
        .map_err(|e: NormalizerError| CaseError::Other(format!("candidate normalize: {e}")))?;
    residuals.extend(compare_observations(
        court_id,
        &o_obs,
        &c_obs,
        companion_model,
    ));

    Ok(residuals)
}

/// The normalizer versions a VM court run used (for the receipt).
pub fn normalizer_versions() -> Vec<(String, String)> {
    vec![
        ("a11y".to_string(), A11Y_NORMALIZER_VERSION.to_string()),
        (
            "candidate".to_string(),
            CANDIDATE_NORMALIZER_VERSION.to_string(),
        ),
        (
            "machine-residual".to_string(),
            RESIDUAL_NORMALIZER_VERSION.to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{CompanionRow, KernelRowObservable};
    use std::collections::BTreeMap;

    fn row(raw: &str, companions: Option<CompanionRow>) -> KernelRowObservable {
        KernelRowObservable {
            raw: raw.into(),
            version: "1.0-1".into(),
            category: "stable".into(),
            checked: false,
            companions,
        }
    }

    fn obs(rows: Vec<KernelRowObservable>) -> Observation {
        Observation {
            rows,
            dialogs: vec![],
        }
    }

    fn model(
        kernel: &str,
        zfs: Option<&str>,
        nvidia: Option<&str>,
    ) -> BTreeMap<String, CompanionExpectation> {
        let mut m = BTreeMap::new();
        m.insert(
            kernel.to_string(),
            CompanionExpectation {
                zfs: zfs.map(|s| s.to_string()),
                nvidia: nvidia.map(|s| s.to_string()),
                nvidia_open: None,
            },
        );
        m
    }

    #[test]
    fn companion_model_match_produces_no_residuals() {
        let oracle = obs(vec![row("fixtures/linux-cachyos-court2", None)]);
        let candidate = obs(vec![row(
            "fixtures/linux-cachyos-court2",
            Some(CompanionRow {
                zfs: Some("linux-cachyos-court2-zfs".into()),
                nvidia: None,
                nvidia_open: None,
            }),
        )]);
        let m = model(
            "fixtures/linux-cachyos-court2",
            Some("linux-cachyos-court2-zfs"),
            None,
        );
        let residuals = compare_observations("court", &oracle, &candidate, &m);
        assert!(residuals.is_empty(), "{residuals:?}");
    }

    #[test]
    fn companion_model_mismatch_produces_residuals() {
        let oracle = obs(vec![row("fixtures/linux-cachyos-court2", None)]);
        let candidate = obs(vec![row(
            "fixtures/linux-cachyos-court2",
            Some(CompanionRow {
                zfs: Some("linux-cachyos-court2-zfs".into()),
                nvidia: None,
                nvidia_open: None,
            }),
        )]);
        // model expects nvidia present but the candidate found none
        let m = model(
            "fixtures/linux-cachyos-court2",
            Some("linux-cachyos-court2-zfs"),
            Some("linux-cachyos-court2-nvidia"),
        );
        let residuals = compare_observations("court", &oracle, &candidate, &m);
        assert!(residuals.iter().any(|r| r.id.contains("companion-nvidia")));
    }

    #[test]
    fn companion_model_ignores_rows_not_in_model() {
        let oracle = obs(vec![row("cachyos/linux-cachyos", None)]);
        let candidate = obs(vec![row("cachyos/linux-cachyos", None)]);
        let m = BTreeMap::new();
        let residuals = compare_observations("court", &oracle, &candidate, &m);
        assert!(residuals.is_empty());
    }
}
