//! `cargo xtask` — Rust-native orchestration.
//!
//! Commands (directive §75):
//! - `oracle verify`        — verify the frozen archive hash against the lock
//! - `oracle info`          — print the frozen authority record
//! - `oracle archive`       — regenerate + verify the deterministic archive
//! - `oracle pkg-hash`      — hash the shipped CachyOS package into the lock
//! - `upstream diff <ref>`  — diff the locked revision against a candidate ref
//! - `court list`           — list all court case directories
//! - `court run <case>`     — run a court (pure or, with `--vm`, differential)
//! - `court run --all`      — run every court whose fixture is present
//! - `vm build`             — build the base VM image (docker + qemu)
//! - `vm bake <fixture>`    — bake a court fixture image
//! - `evidence verify`      — verify all evidence.json hashes

use std::path::Path;
use std::process::{Command, ExitCode};

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR"); // xtask/..
fn repo_root() -> &'static Path {
    Path::new(REPO_ROOT)
        .parent()
        .expect("xtask is a direct member")
}

fn lock_path() -> std::path::PathBuf {
    repo_root().join("oracle/UPSTREAM.lock")
}

fn vm_images() -> std::path::PathBuf {
    repo_root().join("vm/images")
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd} exited with {status}"))
    }
}

fn run_logged(cmd: &str, args: &[&str], log: &Path) -> Result<(), String> {
    let log_file = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let status = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::from(
            log_file.try_clone().map_err(|e| e.to_string())?,
        ))
        .stderr(std::process::Stdio::from(log_file))
        .status()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd} exited with {status}; see {}", log.display()))
    }
}

fn main() -> ExitCode {
    let owned: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    match args.as_slice() {
        [cmd, rest @ ..] if *cmd == "oracle" => match rest {
            ["verify"] => oracle_verify(),
            ["info"] => oracle_info(),
            ["archive"] => oracle_archive(),
            ["pkg-hash"] => oracle_pkg_hash(),
            other => {
                eprintln!("xtask oracle: unknown subcommand {other:?} (expected: verify | info | archive | pkg-hash)");
                ExitCode::FAILURE
            }
        },
        ["upstream", "diff", reference] => upstream_diff(reference),
        ["court", "list"] => court_list(),
        ["court", "run", "--all"] => court_run_all(),
        ["court", "run", case, "--vm"] => court_run_vm(case),
        ["court", "run", case] => court_run(case),
        ["vm", "build"] => vm_build(),
        ["vm", "bake", fixture] => vm_bake(fixture),
        ["evidence", "verify"] => evidence_verify(),
        _ => {
            eprintln!(
                "usage: cargo xtask <oracle verify|info|archive|pkg-hash> | upstream diff <ref> | court list | court run <case> [--vm] | court run --all | vm build | vm bake <fixture> | evidence verify"
            );
            ExitCode::FAILURE
        }
    }
}

fn oracle_verify() -> ExitCode {
    match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(lock) => match lock.verify_archive(repo_root()) {
            Ok(true) => {
                println!(
                    "oracle archive OK: {} ({})",
                    lock.oracle.source_archive, lock.oracle.source_archive_hash
                );
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!("oracle archive MISMATCH: {}", lock.oracle.source_archive);
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("oracle verify error: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("cannot load {}: {e}", lock_path().display());
            ExitCode::FAILURE
        }
    }
}

fn oracle_archive() -> ExitCode {
    // Regenerate the deterministic source archive with `git archive` and
    // verify it against the lock. Deterministic: identical bytes on every
    // run for the same commit.
    let lock = match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot load lock: {e}");
            return ExitCode::FAILURE;
        }
    };
    let upstream = repo_root().join("oracle/upstream");
    if !upstream.join(".git").exists() {
        eprintln!("oracle/upstream is not a git clone");
        return ExitCode::FAILURE;
    }
    let out = repo_root().join(&lock.oracle.source_archive);
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&upstream)
        .arg("archive")
        .arg("--format=tar.gz")
        .arg("-o")
        .arg(&out)
        .arg(&lock.oracle.commit)
        .status();
    match status {
        Ok(s) if s.success() => match lock.verify_archive(repo_root()) {
            Ok(true) => {
                println!(
                    "archive regenerated and verified: {} ({})",
                    lock.oracle.source_archive, lock.oracle.source_archive_hash
                );
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!("archive regenerated but hash MISMATCH — lock and archive disagree");
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("verify error: {e}");
                ExitCode::FAILURE
            }
        },
        Ok(_) => {
            eprintln!("git archive failed");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("git archive error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn oracle_info() -> ExitCode {
    match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(lock) => {
            println!(
                "oracle: {} @ {} ({})\n  commit: {}\n  tree: {}\n  tag: {}\n  retrieved: {}\n  archive: {}\n  archive_sha256: {}\n  polkit_action: {}\n  binary: {}",
                lock.oracle.repository,
                lock.oracle.branch,
                lock.oracle.version,
                lock.oracle.commit,
                lock.oracle.tree,
                lock.oracle.tag.as_deref().unwrap_or("(none)"),
                lock.oracle.retrieved_at,
                lock.oracle.source_archive,
                lock.oracle.source_archive_hash,
                lock.identity.polkit_action,
                lock.identity.binary,
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot load {}: {e}", lock_path().display());
            ExitCode::FAILURE
        }
    }
}

fn upstream_diff(reference: &str) -> ExitCode {
    let lock = match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot load lock: {e}");
            return ExitCode::FAILURE;
        }
    };
    let upstream = repo_root().join("oracle/upstream");
    if !upstream.join(".git").exists() {
        eprintln!("oracle/upstream is not a git clone; run: git clone https://github.com/CachyOS/kernel-manager oracle/upstream");
        return ExitCode::FAILURE;
    }
    match cachyos_kernel_manager_oracle::diff_revisions(&upstream, &lock.oracle.commit, reference) {
        Ok(files) => {
            println!(
                "changed files between {} and {}:",
                lock.oracle.commit, reference
            );
            for f in files {
                println!("  {f}");
            }
            println!("NOTE: review the change queue before accepting a new oracle revision (docs/ORACLE.md).");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("upstream diff error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn court_list() -> ExitCode {
    let courts_root = repo_root().join("courts");
    let mut found = 0;
    let Ok(domains) = std::fs::read_dir(&courts_root) else {
        eprintln!("cannot read courts/: {}", courts_root.display());
        return ExitCode::FAILURE;
    };
    for domain in domains {
        let Ok(domain) = domain else { continue };
        if !domain.path().is_dir() {
            continue;
        }
        let Ok(cases) = std::fs::read_dir(domain.path()) else {
            continue;
        };
        for case in cases {
            let Ok(case) = case else { continue };
            if case.path().join("claim.toml").exists() {
                println!(
                    "{}/{}",
                    domain.file_name().to_string_lossy(),
                    case.file_name().to_string_lossy()
                );
                found += 1;
            }
        }
    }
    println!("{found} courts defined");
    ExitCode::SUCCESS
}

fn court_run(case_id: &str) -> ExitCode {
    let Some((domain, name)) = case_id.split_once('/') else {
        eprintln!("court id must be <domain>/<case>, got {case_id:?}");
        return ExitCode::FAILURE;
    };
    let case = match cachyos_kernel_manager_casefile::Case::load(
        domain,
        name,
        &repo_root().join("courts"),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot load court {case_id}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match case.compare() {
        Ok(residuals) if residuals.is_empty() => {
            println!("court {case_id}: PASS (fixture+oracle+candidate fingerprints identical)");
            ExitCode::SUCCESS
        }
        Ok(residuals) => {
            println!("court {case_id}: FAIL — {} residual(s):", residuals.len());
            for r in &residuals {
                println!(
                    "  [{}] {} oracle={} candidate={}",
                    r.classification, r.id, r.oracle_fingerprint, r.candidate_fingerprint
                );
            }
            let residual_json = serde_json::to_string_pretty(&residuals).unwrap();
            let target = case.dir.join("residual.json");
            std::fs::write(&target, residual_json).expect("write residual.json");
            println!("  residual ledger written to {}", target.display());
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("court compare error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn court_run_all() -> ExitCode {
    let courts_root = repo_root().join("courts");
    let mut failed = Vec::new();
    let mut ran = 0;
    for domain in std::fs::read_dir(&courts_root).expect("courts/") {
        let Ok(domain) = domain else { continue };
        if !domain.path().is_dir() {
            continue;
        }
        for case in std::fs::read_dir(domain.path()).expect("domain") {
            let Ok(case) = case else { continue };
            if !case.path().join("claim.toml").exists() {
                continue;
            }
            let id = format!(
                "{}/{}",
                domain.file_name().to_string_lossy(),
                case.file_name().to_string_lossy()
            );
            ran += 1;
            if court_run(&id) != ExitCode::SUCCESS {
                failed.push(id);
            }
        }
    }
    println!("courts: {ran} run, {} failed", failed.len());
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ---------------------------------------------------------------------------
// Phase 2: VM oracle architecture
// ---------------------------------------------------------------------------

/// `oracle pkg-hash` — download the shipped CachyOS package at freeze time,
/// hash it, and record it in the lock (`package_hashes`).
fn oracle_pkg_hash() -> ExitCode {
    let lock = match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot load lock: {e}");
            return ExitCode::FAILURE;
        }
    };
    let url = format!(
        "https://mirror.cachyos.org/repo/x86_64/cachyos/{}-{}-1-x86_64.pkg.tar.zst",
        lock.identity.binary, lock.oracle.version
    );
    let tmp = std::env::temp_dir().join("cachyos-km-pkg.tar.zst");
    println!("downloading {url}");
    let status = Command::new("curl")
        .args(["-fsSL", "-o", tmp.to_str().expect("tmp path"), &url])
        .status()
        .expect("curl");
    if !status.success() {
        eprintln!("download failed; is {url} still the shipped version?");
        return ExitCode::FAILURE;
    }
    let bytes = std::fs::read(&tmp).expect("read download");
    let hash = cachyos_kernel_manager_casefile::sha256_bytes(&bytes);
    println!("{} = sha256:{hash}", lock.identity.binary);

    // comment-preserving lock edit: keep package_hashes as an INLINE table
    // (a full Item::Table would be rendered as a section header and mangle
    // the comments inside [oracle])
    let content = std::fs::read_to_string(lock_path()).expect("lock read");
    let mut doc: toml_edit::DocumentMut = content.parse().expect("lock toml");
    let key = format!("{}-{}-1", lock.identity.binary, lock.oracle.version);
    let mut inline = toml_edit::InlineTable::new();
    inline.insert(
        &key,
        toml_edit::value(format!("sha256:{hash}"))
            .into_value()
            .expect("value"),
    );
    doc["oracle"]["package_hashes"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(inline));
    std::fs::write(lock_path(), doc.to_string()).expect("lock write");
    println!("package_hashes recorded in oracle/UPSTREAM.lock");
    ExitCode::SUCCESS
}

/// `vm build` — build the base VM image (runs vm/base/build-base.sh),
/// then record `reference_image_hash` into the lock.
fn vm_build() -> ExitCode {
    let images = vm_images();
    let base_qcow2 = images.join("base.qcow2");
    let manifest_path = images.join("manifest.json");

    if !base_qcow2.exists() {
        println!("base image missing; running the docker+qemu builder (this takes a while)");
        let log = images.join("build-base.log");
        if let Err(e) = run_logged("bash", &["vm/base/build-base.sh"], &log) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    } else {
        println!("base image present: {}", base_qcow2.display());
    }

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("manifest.json"))
            .expect("manifest parse");
    let Some(hash) = manifest
        .get("reference_image_hash")
        .and_then(|v| v.as_str())
    else {
        eprintln!("manifest.json has no reference_image_hash");
        return ExitCode::FAILURE;
    };

    let content = std::fs::read_to_string(lock_path()).expect("lock read");
    let mut doc: toml_edit::DocumentMut = content.parse().expect("lock toml");
    doc["oracle"]["reference_image_hash"] = toml_edit::value(hash.to_string());
    std::fs::write(lock_path(), doc.to_string()).expect("lock write");
    println!("reference_image_hash = {hash} (recorded in UPSTREAM.lock)");
    ExitCode::SUCCESS
}

/// `vm bake <fixture>` — bake a fixture image.
fn vm_bake(fixture: &str) -> ExitCode {
    let images = vm_images();
    if !images.join("base.qcow2").exists() {
        eprintln!("base.qcow2 missing — run: cargo xtask vm build");
        return ExitCode::FAILURE;
    }
    if !images.join("base-rootfs").is_dir() {
        eprintln!("base-rootfs/ missing — run: cargo xtask vm build");
        return ExitCode::FAILURE;
    }
    let out = images.join("fixtures").join(fixture).join("fixture.qcow2");
    if out.exists() {
        println!("fixture {fixture} already baked: {}", out.display());
        return ExitCode::SUCCESS;
    }
    match run("bash", &["vm/fixtures/bake.sh", fixture]) {
        Ok(()) => {
            println!("fixture {fixture} ready: {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// `court run <case> --vm` — full differential court:
/// bake fixture -> fresh overlay -> oracle observe -> fresh overlay ->
/// candidate observe -> compare -> residual + evidence.
fn court_run_vm(case_id: &str) -> ExitCode {
    let Some((domain, name)) = case_id.split_once('/') else {
        eprintln!("court id must be <domain>/<case>, got {case_id:?}");
        return ExitCode::FAILURE;
    };
    let courts_root = repo_root().join("courts");
    let case = match cachyos_kernel_manager_casefile::Case::load(domain, name, &courts_root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot load court {case_id}: {e}");
            return ExitCode::FAILURE;
        }
    };

    // fixture selection: comparator.toml `fixture` field, else the case name
    let fixture = case
        .comparator
        .fixture
        .clone()
        .unwrap_or_else(|| name.to_string());
    let fixture_img = vm_images()
        .join("fixtures")
        .join(&fixture)
        .join("fixture.qcow2");
    if !fixture_img.exists() {
        eprintln!(
            "fixture image missing: {} (run: cargo xtask vm bake {fixture})",
            fixture_img.display()
        );
        return ExitCode::FAILURE;
    }

    let vm_ctl = repo_root().join("vm/harness/vm-ctl.sh");
    let overlay_dir = vm_images().join("overlays");
    std::fs::create_dir_all(&overlay_dir).expect("overlay dir");
    let oracle_dir = case.dir.join("oracle");
    let candidate_dir = case.dir.join("candidate");
    std::fs::create_dir_all(&oracle_dir).expect("oracle dir");
    std::fs::create_dir_all(&candidate_dir).expect("candidate dir");

    // candidate inspect binary into the share
    let share = vm_images().join("share");
    let inspect_src = repo_root().join("target/debug/cachyos-kernel-manager-inspect");
    if !inspect_src.exists() {
        eprintln!("build the inspect tool first: cargo build -p cachyos-kernel-manager-alpm --features libalpm --bin cachyos-kernel-manager-inspect");
        return ExitCode::FAILURE;
    }
    std::fs::create_dir_all(share.join("inspect")).expect("share/inspect");
    std::fs::copy(
        &inspect_src,
        share.join("inspect/cachyos-kernel-manager-inspect"),
    )
    .expect("copy inspect");
    // iterate without rebaking: fresh in-VM scripts via the 9p share
    let scripts_dst = share.join("scripts");
    let _ = std::fs::remove_dir_all(&scripts_dst);
    std::fs::create_dir_all(&scripts_dst).expect("share/scripts");
    for entry in std::fs::read_dir(repo_root().join("vm/in-vm")).expect("vm/in-vm") {
        let entry = entry.expect("entry");
        std::fs::copy(entry.path(), scripts_dst.join(entry.file_name()))
            .expect("copy in-vm script");
    }

    let run_side = |side: &str, out_dir: &std::path::Path| -> Result<(), String> {
        let overlay = overlay_dir.join(format!("{}-{}.qcow2", name, side));
        let _ = std::fs::remove_file(&overlay);
        run(
            "qemu-img",
            &[
                "create",
                "-f",
                "qcow2",
                "-F",
                "qcow2",
                "-b",
                fixture_img.to_str().expect("path"),
                overlay.to_str().expect("path"),
            ],
        )?;
        // fresh share/out for this side
        let share_out = share.join("out");
        let _ = std::fs::remove_dir_all(&share_out);
        std::fs::create_dir_all(&share_out).map_err(|e| e.to_string())?;
        let ctl = vm_ctl.to_str().expect("path");
        // stale qemu from a killed previous run holds the hostfwd port and/or
        // the overlay; clean it before every start (fail-fast guard in
        // vm-ctl.sh catches anything that survives this)
        let _ = run("bash", &[ctl, "cleanup"]);
        run("bash", &[ctl, "start", overlay.to_str().expect("path")])?;
        let res = (|| -> Result<(), String> {
            match side {
                "oracle" => {
                    // exec the SHARE copy of the observer (the image copy is
                    // the fallback for standalone use; the share carries the
                    // current revision without rebaking)
                    run(
                        "bash",
                        &[
                            ctl,
                            "exec",
                            "bash /mnt/host/scripts/oracle-observe.sh /mnt/host/out",
                        ],
                    )?;
                }
                "candidate" => {
                    run(
                        "bash",
                        &[
                            ctl,
                            "exec",
                            "bash /mnt/host/scripts/candidate-observe.sh /mnt/host/out",
                        ],
                    )?;
                }
                _ => unreachable!(),
            }
            Ok(())
        })();
        let _ = run("bash", &[ctl, "stop"]);
        res?;
        // pull observations out of the share into the case dir
        for entry in std::fs::read_dir(&share_out).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let dest = out_dir.join(entry.file_name());
            std::fs::copy(entry.path(), &dest).map_err(|e| e.to_string())?;
        }
        Ok(())
    };

    if let Err(e) = run_side("oracle", &oracle_dir) {
        eprintln!("oracle run failed: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = run_side("candidate", &candidate_dir) {
        eprintln!("candidate run failed: {e}");
        return ExitCode::FAILURE;
    }

    // rename candidate residual to avoid clashing with the oracle's
    let _ = std::fs::rename(
        candidate_dir.join("residual.json"),
        candidate_dir.join("candidate-residual.json"),
    );

    // compare
    let residuals = match cachyos_kernel_manager_casefile::vm_court::compare_vm_observations(
        &case.dir,
        case_id,
        &case.comparator.companion_model,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("comparison error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // write residual.json + evidence.json
    let residual_json = serde_json::to_string_pretty(&residuals).expect("serialize");
    std::fs::write(case.dir.join("residual.json"), &residual_json).expect("write residual");

    let lock = cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path())
        .map(|l| l.oracle.commit)
        .unwrap_or_else(|_| "unknown".to_string());
    let fixture_manifest_path = vm_images()
        .join("fixtures")
        .join(&fixture)
        .join("fixture-manifest.json");
    let fixture_digest = std::fs::read_to_string(&fixture_manifest_path)
        .ok()
        .and_then(|s| {
            serde_json::from_str::<serde_json::Value>(&s)
                .ok()
                .and_then(|v| {
                    v.get("fixture_digest")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string())
                })
        });

    let mut evidence = cachyos_kernel_manager_casefile::evidence::EvidenceRecord {
        court: case_id.to_string(),
        oracle_revision: lock.clone(),
        candidate_revision: env!("CARGO_PKG_VERSION").to_string(),
        fixture_digest,
        normalizers: cachyos_kernel_manager_casefile::vm_court::normalizer_versions(),
        comparator_version: cachyos_kernel_manager_casefile::vm_court::COMPARATOR_VERSION
            .to_string(),
        result: if residuals.is_empty() {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        residual_count: residuals.len(),
        artifacts: vec![],
    };
    let _ = evidence.add_directory(&oracle_dir, "oracle");
    let _ = evidence.add_directory(&candidate_dir, "candidate");
    // the fixture qcow2/raw (20 GB) is captured by fixture_digest; copy the
    // small metadata files INTO the case dir so the evidence is
    // self-contained and verifiable after cloning the repo elsewhere
    let fixture_dir = fixture_manifest_path.parent().expect("parent");
    let case_fixture_dir = case.dir.join("fixture");
    let _ = std::fs::create_dir_all(&case_fixture_dir);
    let _ = std::fs::copy(
        &fixture_manifest_path,
        case_fixture_dir.join("fixture-manifest.json"),
    );
    let _ = std::fs::copy(
        fixture_dir.join("packages.txt"),
        case_fixture_dir.join("packages.txt"),
    );
    let _ = evidence.add_directory(&case_fixture_dir, "fixture");
    let _ = evidence.write(&case.dir);

    if residuals.is_empty() {
        println!("court {case_id}: PASS (oracle == candidate on fixture {fixture})");
        ExitCode::SUCCESS
    } else {
        println!("court {case_id}: FAIL — {} residual(s):", residuals.len());
        for r in &residuals {
            println!(
                "  [{}] {} oracle={} candidate={}",
                r.classification, r.id, r.oracle_fingerprint, r.candidate_fingerprint
            );
        }
        ExitCode::FAILURE
    }
}

/// `evidence verify` — verify every evidence.json in courts/ against the
/// filesystem (content-addressed integrity, directive §77).
fn evidence_verify() -> ExitCode {
    let courts_root = repo_root().join("courts");
    let mut bad = Vec::new();
    let mut checked = 0;
    for domain in std::fs::read_dir(&courts_root).expect("courts/") {
        let Ok(domain) = domain else { continue };
        if !domain.path().is_dir() {
            continue; // courts/ also holds tooling scripts
        }
        for case in std::fs::read_dir(domain.path()).expect("domain") {
            let Ok(case) = case else { continue };
            if !case.path().is_dir() {
                continue;
            }
            let ev_path = case.path().join("evidence.json");
            if !ev_path.exists() {
                continue;
            }
            checked += 1;
            match cachyos_kernel_manager_casefile::evidence::EvidenceRecord::load(&case.path()) {
                Ok(ev) => match ev.verify(&case.path()) {
                    Ok(mismatches) if mismatches.is_empty() => {
                        println!(
                            "evidence OK: {}/{}",
                            domain.file_name().to_string_lossy(),
                            case.file_name().to_string_lossy()
                        );
                    }
                    Ok(mismatches) => {
                        bad.push(format!(
                            "{}/{}",
                            domain.file_name().to_string_lossy(),
                            case.file_name().to_string_lossy()
                        ));
                        for m in mismatches {
                            println!("  MISMATCH: {m}");
                        }
                    }
                    Err(e) => bad.push(format!(
                        "{}/{}: {e}",
                        domain.file_name().to_string_lossy(),
                        case.file_name().to_string_lossy()
                    )),
                },
                Err(e) => bad.push(format!(
                    "{}/{}: {e}",
                    domain.file_name().to_string_lossy(),
                    case.file_name().to_string_lossy()
                )),
            }
        }
    }
    println!("evidence verified: {checked} records");
    if bad.is_empty() {
        ExitCode::SUCCESS
    } else {
        for b in &bad {
            eprintln!("evidence problem: {b}");
        }
        ExitCode::FAILURE
    }
}
