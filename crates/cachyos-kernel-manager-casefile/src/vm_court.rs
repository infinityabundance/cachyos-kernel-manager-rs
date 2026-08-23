//! VM court comparison: align oracle and candidate observations, compare,
//! and produce the residual + evidence records (directive §44, §45, §46).

#![forbid(unsafe_code)]

use crate::normalize::{
    candidate_observation, candidate_transaction_observation, oracle_observation,
    oracle_transaction_observation, residual_digest, terminal_matrix_observation, NormalizerError,
    Observation, TransactionObservation, A11Y_NORMALIZER_VERSION, CANDIDATE_NORMALIZER_VERSION,
    RESIDUAL_NORMALIZER_VERSION, TERMINAL_MATRIX_NORMALIZER_VERSION,
    TRANSACTION_NORMALIZER_VERSION,
};
use crate::CaseError;
use cachyos_kernel_manager_frf::Residual;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// sha256 hex of a byte slice.
fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Comparator version (bump on comparison-semantics changes).
pub const COMPARATOR_VERSION: &str = "1.2.0";

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

/// Phase 9 gap-006 court (build-env/makepkg-runtime): compare the two VM
/// boots' makepkg runtime witnesses byte-for-byte.
///
/// - `commands.txt` (the oracle's frozen literals vs the candidate's model
///   render — must be IDENTICAL);
/// - `<scenario>-execs.txt` (the normalized execve chains: scf, sicf,
///   aurfail);
/// - `makepkg-version.txt`;
/// - the machine residual: the `packages.txt` digest on both sides (the
///   fixture-integrity check — both boots must end in the same package
///   state).
///
/// The `<scenario>-raw.trace` files are the immutable raw evidence (never
/// compared byte-exact — they contain PIDs; the normalized extraction is
/// the compared observable).
pub fn compare_makepkg(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    // 1. machine residual: the packages.txt digest (fixture-integrity)
    let o_pkgs = std::fs::read(oracle_dir.join("packages.txt"))?;
    let c_pkgs = std::fs::read(candidate_dir.join("packages.txt"))?;
    let o_digest = sha256_bytes(&o_pkgs);
    let c_digest = sha256_bytes(&c_pkgs);
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

    // 2. the compared observables (byte-exact)
    for name in [
        "commands.txt",
        "scf-execs.txt",
        "sicf-execs.txt",
        "aurfail-execs.txt",
        "makepkg-version.txt",
    ] {
        let o = std::fs::read(oracle_dir.join(name))?;
        let c = std::fs::read(candidate_dir.join(name))?;
        if o != c {
            residuals.push(Residual {
                id: format!("{court_id}-{name}"),
                court: court_id.into(),
                layer: "exec-chain".into(),
                oracle_fingerprint: sha256_bytes(&o),
                candidate_fingerprint: sha256_bytes(&c),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            });
        }
    }

    Ok(residuals)
}

/// Phase 11 boot court (boot/system-boot-after-install --vm): compare the
/// two boots' kernel-install + reboot surfaces byte-for-byte. Both boots
/// run the IDENTICAL install + reboot sequence; every written file must
/// match (determinism + the mutation's stability): the install exec chain,
/// the install command (the model render vs the frozen literal — must be
/// IDENTICAL), the pre/post kernel + /boot states, the hook output, the
/// post-reboot boot status, the running kernel, and the machine residual.
pub fn compare_boot(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    // the machine residual (fixture-integrity)
    let o_pkgs = std::fs::read(oracle_dir.join("packages.txt"))?;
    let c_pkgs = std::fs::read(candidate_dir.join("packages.txt"))?;
    if o_pkgs != c_pkgs {
        residuals.push(Residual {
            id: format!("{court_id}-machine-residual-drift"),
            court: court_id.into(),
            layer: "machine-residual".into(),
            oracle_fingerprint: sha256_bytes(&o_pkgs),
            candidate_fingerprint: sha256_bytes(&c_pkgs),
            classification: "fixture_drift".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    // every other file byte-exact (the raw strace traces contain PIDs —
    // evidence only, never compared)
    let mut names: Vec<String> = std::fs::read_dir(&oracle_dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()))
        .collect::<Result<_, _>>()?;
    names.sort();
    for name in names {
        if name == "packages.txt" || name == "install-raw.trace" || name == "remove-raw.trace" {
            continue;
        }
        let o = std::fs::read(oracle_dir.join(&name))?;
        let c = std::fs::read(candidate_dir.join(&name))?;
        if o != c {
            residuals.push(Residual {
                id: format!("{court_id}-{name}"),
                court: court_id.into(),
                layer: "boot-surface".into(),
                oracle_fingerprint: sha256_bytes(&o),
                candidate_fingerprint: sha256_bytes(&c),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            });
        }
    }

    Ok(residuals)
}

/// Phase 10 packaging court (packaging/upgrade --vm): compare the two
/// boots' package-transition surfaces byte-for-byte. Both boots run the
/// IDENTICAL oracle->candidate->oracle transition script; every written
/// file must match (determinism + the transition's stability):
/// baseline/upgraded/reverted versions, the file lists, the `--version`
/// flags, the discovery row names, and the machine residual.
pub fn compare_packaging(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    // the machine residual (fixture-integrity)
    let o_pkgs = std::fs::read(oracle_dir.join("packages.txt"))?;
    let c_pkgs = std::fs::read(candidate_dir.join("packages.txt"))?;
    if o_pkgs != c_pkgs {
        residuals.push(Residual {
            id: format!("{court_id}-machine-residual-drift"),
            court: court_id.into(),
            layer: "machine-residual".into(),
            oracle_fingerprint: sha256_bytes(&o_pkgs),
            candidate_fingerprint: sha256_bytes(&c_pkgs),
            classification: "fixture_drift".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    // every other file (the transition surfaces) byte-exact
    let mut names: Vec<String> = std::fs::read_dir(&oracle_dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()))
        .collect::<Result<_, _>>()?;
    names.sort();
    for name in names {
        if name == "packages.txt" {
            continue;
        }
        let o = std::fs::read(oracle_dir.join(&name))?;
        let c = std::fs::read(candidate_dir.join(&name))?;
        if o != c {
            residuals.push(Residual {
                id: format!("{court_id}-{name}"),
                court: court_id.into(),
                layer: "packaging-surface".into(),
                oracle_fingerprint: sha256_bytes(&o),
                candidate_fingerprint: sha256_bytes(&c),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            });
        }
    }

    Ok(residuals)
}

/// Phase 12 production-integration slice (ui/gui-drive --vm): compare the
/// semantic sort/toggle sequence + the machine residuals byte-for-byte.
/// Both sides produce drive-semantic.json (the sorted pkgname order per
/// header + the toggled identity) — the oracle from its Qt tree, the
/// candidate from its own courted KM_VERBOSE trace — and the residuals
/// (residual.json vs candidate-residual.json, the runner's rename).
pub fn compare_gui_drive(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    // the machine residual (fixture-integrity): both sides run on fresh
    // overlays of the SAME fixture image
    let oracle_residual = std::fs::read(oracle_dir.join("residual.json"))?;
    let candidate_residual = std::fs::read(candidate_dir.join("candidate-residual.json"))
        .or_else(|_| std::fs::read(candidate_dir.join("residual.json")))?;
    if oracle_residual != candidate_residual {
        residuals.push(Residual {
            id: format!("{court_id}-machine-residual-drift"),
            court: court_id.into(),
            layer: "machine-residual".into(),
            oracle_fingerprint: sha256_bytes(&oracle_residual),
            candidate_fingerprint: sha256_bytes(&candidate_residual),
            classification: "fixture_drift".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    // the semantic sequence: the sorted pkgname order per header + the
    // toggled identity — the court's claim (the toggle followed the kernel
    // identity through the reorder, never a presentation index)
    let oracle_sem = std::fs::read(oracle_dir.join("drive-semantic.json"))?;
    let candidate_sem = std::fs::read(candidate_dir.join("drive-semantic.json"))?;
    if oracle_sem != candidate_sem {
        residuals.push(Residual {
            id: format!("{court_id}-semantic-sequence"),
            court: court_id.into(),
            layer: "gui-drive-semantic".into(),
            oracle_fingerprint: sha256_bytes(&oracle_sem),
            candidate_fingerprint: sha256_bytes(&candidate_sem),
            classification: "deterministic_mismatch".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    Ok(residuals)
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

/// Compare two transaction observations chain-by-chain, order-sensitive.
pub fn compare_transaction_observations(
    court: &str,
    oracle: &TransactionObservation,
    candidate: &TransactionObservation,
) -> Vec<Residual> {
    let mut residuals = Vec::new();
    for (field, o, c) in [
        ("probes", &oracle.probes, &candidate.probes),
        ("execs", &oracle.execs, &candidate.execs),
    ] {
        if o != c {
            residuals.push(Residual {
                id: format!("{court}-transaction-{field}"),
                court: court.into(),
                layer: "transaction-chain".into(),
                oracle_fingerprint: format!("{o:?}"),
                candidate_fingerprint: format!("{c:?}"),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            });
        }
    }
    if oracle.terminal != candidate.terminal {
        residuals.push(Residual {
            id: format!("{court}-transaction-terminal"),
            court: court.into(),
            layer: "transaction-chain".into(),
            oracle_fingerprint: format!("{:?}", oracle.terminal),
            candidate_fingerprint: format!("{:?}", candidate.terminal),
            classification: "deterministic_mismatch".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }
    residuals
}

/// Run the transaction-chain comparison for a court case directory:
/// `oracle/oracle-transaction.json` vs `candidate/candidate-transaction.json`.
pub fn compare_vm_transactions(
    case_dir: &Path,
    court_id: &str,
) -> Result<Vec<Residual>, CaseError> {
    let oracle_path = case_dir.join("oracle").join("oracle-transaction.json");
    let candidate_path = case_dir
        .join("candidate")
        .join("candidate-transaction.json");
    if !oracle_path.exists() || !candidate_path.exists() {
        return Err(CaseError::Other(format!(
            "transaction court requires oracle/oracle-transaction.json and \
             candidate/candidate-transaction.json (missing: {} / {})",
            oracle_path.exists(),
            candidate_path.exists()
        )));
    }
    let o = read_json(&oracle_path)?;
    let c = read_json(&candidate_path)?;
    let o_obs = oracle_transaction_observation(&o)
        .map_err(|e: NormalizerError| CaseError::Other(format!("oracle tx normalize: {e}")))?;
    let c_obs = candidate_transaction_observation(&c)
        .map_err(|e: NormalizerError| CaseError::Other(format!("candidate tx normalize: {e}")))?;
    Ok(compare_transaction_observations(court_id, &o_obs, &c_obs))
}

/// Compare the mutation-court PKGBUILD evidence (`patch-injection/*`,
/// `custom-name/*`):
///   1. the oracle's pre-mutation PKGBUILD must equal the candidate's
///      pre-mutation PKGBUILD (both from fresh overlays of the same fixture;
///      proves the git refresh was a no-op and both sides mutated identical
///      input text),
///   2. the oracle's post-mutation PKGBUILD must equal the candidate's
///      modeled mutation byte-for-byte.
pub fn compare_mutation(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");
    let mut residuals = Vec::new();

    let ob = std::fs::read_to_string(oracle_dir.join("pkgbuild-before.txt"))?;
    let cb = std::fs::read_to_string(candidate_dir.join("candidate-pkgbuild-before.txt"))?;
    if ob != cb {
        residuals.push(Residual {
            id: format!("{court_id}-pkgbuild-before"),
            court: court_id.into(),
            layer: "pkgbuild-mutation".into(),
            oracle_fingerprint: sha256_of(&ob),
            candidate_fingerprint: sha256_of(&cb),
            classification: "deterministic_mismatch".into(),
            root_cause: Some("the pre-mutation PKGBUILDs differ — the git refresh was not a no-op or the fixture overlays diverge".into()),
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }

    let oa = std::fs::read_to_string(oracle_dir.join("pkgbuild-after.txt"))?;
    let ca = std::fs::read_to_string(candidate_dir.join("candidate-pkgbuild-after.txt"))?;
    if oa != ca {
        residuals.push(Residual {
            id: format!("{court_id}-pkgbuild-after"),
            court: court_id.into(),
            layer: "pkgbuild-mutation".into(),
            oracle_fingerprint: sha256_of(&oa),
            candidate_fingerprint: sha256_of(&ca),
            classification: "deterministic_mismatch".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }
    Ok(residuals)
}

fn sha256_of(s: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compare the terminal-matrix observations (schema
/// `cachyos-km-terminal-matrix-v1`): every scenario's exit code, stdout
/// (temp paths normalized), stderr, and tmp-file-leftover count must match.
pub fn compare_terminal_matrix(
    case_dir: &Path,
    court_id: &str,
) -> Result<Vec<Residual>, CaseError> {
    let oracle_path = case_dir.join("oracle").join("terminal-matrix.json");
    let candidate_path = case_dir.join("candidate").join("terminal-matrix.json");
    if !oracle_path.exists() || !candidate_path.exists() {
        return Err(CaseError::Other(format!(
            "terminal-matrix court requires oracle/terminal-matrix.json and \
             candidate/terminal-matrix.json (missing: {} / {})",
            oracle_path.exists(),
            candidate_path.exists()
        )));
    }
    let o = read_json(&oracle_path)?;
    let c = read_json(&candidate_path)?;
    let o_norm = terminal_matrix_observation(&o)
        .map_err(|e: NormalizerError| CaseError::Other(format!("oracle matrix normalize: {e}")))?;
    let c_norm = terminal_matrix_observation(&c).map_err(|e: NormalizerError| {
        CaseError::Other(format!("candidate matrix normalize: {e}"))
    })?;
    let mut residuals = Vec::new();
    if o_norm != c_norm {
        residuals.push(Residual {
            id: format!("{court_id}-terminal-matrix"),
            court: court_id.into(),
            layer: "terminal-matrix".into(),
            oracle_fingerprint: serde_json::to_string(&o_norm).unwrap_or_default(),
            candidate_fingerprint: serde_json::to_string(&c_norm).unwrap_or_default(),
            classification: "deterministic_mismatch".into(),
            root_cause: None,
            resolution: None,
            commit: None,
            regression_witness: None,
        });
    }
    Ok(residuals)
}

/// The Phase 7 scx court comparison: the candidate's typed org.scx.Loader
/// surface must be a FAITHFUL SUBSET of the REAL loader's interface (the
/// reference image's scx_loader is a LATER version than the frozen 1.0.9,
/// so it may expose MORE methods/properties — the frozen oracle only calls
/// its own surface), and the candidate's property readback must equal the
/// real loader's property values.
///
/// Reads:
/// - oracle/introspect.txt      — raw `busctl introspect org.scx.Loader`
/// - oracle/oracle-properties.json — every loader property value
/// - candidate/candidate-interface.json — the candidate's declared interface
/// - candidate/candidate-readback.json  — the candidate's property readback
pub fn compare_scx_interface(case_dir: &Path, court_id: &str) -> Result<Vec<Residual>, CaseError> {
    let mut residuals = Vec::new();
    let oracle_dir = case_dir.join("oracle");
    let candidate_dir = case_dir.join("candidate");

    let introspect = std::fs::read_to_string(oracle_dir.join("introspect.txt"))?;
    let oracle_props: Value = read_json(&oracle_dir.join("oracle-properties.json"))?;
    let candidate_iface: Value = read_json(&candidate_dir.join("candidate-interface.json"))?;
    let candidate_readback: Value = read_json(&candidate_dir.join("candidate-readback.json"))?;

    // 1. parse the real interface from the introspect text: for every
    //    org.scx.Loader member, (name, kind, signature, access).
    let mut real_methods: Vec<(String, String)> = Vec::new(); // (name, in-sig)
    let mut real_props: Vec<(String, String, bool)> = Vec::new(); // (name, sig, writable)
    for line in introspect.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 || !fields[0].starts_with('.') {
            continue;
        }
        let name = fields[0][1..].to_string();
        match fields[1] {
            "method" => {
                let sig = if fields[2] == "-" {
                    "".to_string()
                } else {
                    fields[2].to_string()
                };
                real_methods.push((name, sig));
            }
            "property" => {
                let sig = fields[2].to_string();
                let flags = fields.get(3..).unwrap_or(&[]).join(" ");
                let writable = flags.contains("readwrite") || flags.contains("writable");
                real_props.push((name, sig, writable));
            }
            _ => {}
        }
    }

    // 2. every candidate method must exist on the real loader with the
    //    same input signature.
    if let Some(methods) = candidate_iface.get("methods").and_then(|m| m.as_array()) {
        for m in methods {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let sig: String = m
                .get("in_args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.get("type").and_then(|t| t.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            if !real_methods.iter().any(|(n, s)| n == name && s == &sig) {
                residuals.push(Residual {
                    id: format!("{court_id}-method-{name}"),
                    court: court_id.into(),
                    layer: "scx-interface".into(),
                    oracle_fingerprint: "absent on the real loader".into(),
                    candidate_fingerprint: format!("{name}({sig})"),
                    classification: "deterministic_mismatch".into(),
                    root_cause: None,
                    resolution: None,
                    commit: None,
                    regression_witness: None,
                });
            }
        }
    }

    // 3. every candidate property must exist on the real loader with the
    //    same signature and the same access (the frozen surface is read-only;
    //    a writable real property with the same name/sig is still compatible,
    //    but a signature or access difference is a real residual).
    if let Some(props) = candidate_iface.get("properties").and_then(|p| p.as_array()) {
        for p in props {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let sig = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let access = p.get("access").and_then(|v| v.as_str()).unwrap_or("");
            let candidate_writable = access == "readwrite" || access == "write";
            match real_props.iter().find(|(n, _, _)| n == name) {
                Some((_, real_sig, real_writable)) => {
                    if real_sig != sig || *real_writable != candidate_writable {
                        residuals.push(Residual {
                            id: format!("{court_id}-property-{name}"),
                            court: court_id.into(),
                            layer: "scx-interface".into(),
                            oracle_fingerprint: format!(
                                "{name}:{real_sig} writable={real_writable}"
                            ),
                            candidate_fingerprint: format!(
                                "{name}:{sig} writable={candidate_writable}"
                            ),
                            classification: "deterministic_mismatch".into(),
                            root_cause: None,
                            resolution: None,
                            commit: None,
                            regression_witness: None,
                        });
                    }
                }
                None => residuals.push(Residual {
                    id: format!("{court_id}-property-{name}"),
                    court: court_id.into(),
                    layer: "scx-interface".into(),
                    oracle_fingerprint: "absent on the real loader".into(),
                    candidate_fingerprint: format!("{name}:{sig}"),
                    classification: "deterministic_mismatch".into(),
                    root_cause: None,
                    resolution: None,
                    commit: None,
                    regression_witness: None,
                }),
            }
        }
    }

    // 4. the state readback: the candidate's property values must equal the
    //    real loader's (both read the same bus).
    let pairs = [
        ("current_scheduler", "CurrentScheduler"),
        ("scheduler_mode", "SchedulerMode"),
        ("supported_schedulers", "SupportedSchedulers"),
    ];
    for (candidate_key, oracle_key) in pairs {
        let o = oracle_props.get(oracle_key);
        let c = candidate_readback.get(candidate_key);
        match (o, c) {
            (Some(o), Some(c)) if o != c => residuals.push(Residual {
                id: format!("{court_id}-readback-{candidate_key}"),
                court: court_id.into(),
                layer: "scx-readback".into(),
                oracle_fingerprint: o.to_string(),
                candidate_fingerprint: c.to_string(),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            }),
            (Some(_), Some(_)) => {}
            (None, Some(c)) => residuals.push(Residual {
                id: format!("{court_id}-readback-{candidate_key}"),
                court: court_id.into(),
                layer: "scx-readback".into(),
                oracle_fingerprint: "property missing from the real loader".into(),
                candidate_fingerprint: c.to_string(),
                classification: "deterministic_mismatch".into(),
                root_cause: None,
                resolution: None,
                commit: None,
                regression_witness: None,
            }),
            (Some(o), None) => residuals.push(Residual {
                id: format!("{court_id}-readback-{candidate_key}"),
                court: court_id.into(),
                layer: "scx-readback".into(),
                oracle_fingerprint: o.to_string(),
                candidate_fingerprint: "missing from the candidate readback".into(),
                classification: "deterministic_mismatch".into(),
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
        (
            "transaction".to_string(),
            TRANSACTION_NORMALIZER_VERSION.to_string(),
        ),
        (
            "terminal-matrix".to_string(),
            TERMINAL_MATRIX_NORMALIZER_VERSION.to_string(),
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

    #[test]
    fn transaction_comparison_is_order_sensitive() {
        let make = |execs: Vec<Vec<&str>>| TransactionObservation {
            probes: vec![vec![
                "sh".into(),
                "-c".into(),
                "findmnt -ln -o FSTYPE /".into(),
            ]],
            execs: execs
                .into_iter()
                .map(|e| e.into_iter().map(|s| s.to_string()).collect())
                .collect(),
            terminal: Some(vec!["terminal-helper".into()]),
        };
        let o = make(vec![vec!["pacman", "-S", "--needed", "a"]]);
        let c_same = make(vec![vec!["pacman", "-S", "--needed", "a"]]);
        assert!(compare_transaction_observations("court", &o, &c_same).is_empty());
        // different package order -> residual
        let c_diff = make(vec![vec!["pacman", "-S", "--needed", "b"]]);
        assert!(!compare_transaction_observations("court", &o, &c_diff).is_empty());
        // extra command -> residual
        let c_extra = make(vec![
            vec!["pacman", "-S", "--needed", "a"],
            vec!["pacman", "-Rsn", "a"],
        ]);
        assert!(!compare_transaction_observations("court", &o, &c_extra).is_empty());
    }

    #[test]
    fn transaction_probe_order_mismatch_is_a_residual() {
        let make = |first: &str| TransactionObservation {
            probes: vec![
                vec!["sh".into(), "-c".into(), first.into()],
                vec!["sh".into(), "-c".into(), "chwd --list-installed -d".into()],
            ],
            execs: vec![],
            terminal: None,
        };
        let o = make("findmnt -ln -o FSTYPE /");
        let c = make("chwd --list-installed -d");
        assert!(!compare_transaction_observations("court", &o, &c).is_empty());
    }
}
