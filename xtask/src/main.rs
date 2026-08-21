//! `cargo xtask` — Rust-native orchestration.
//!
//! Commands (directive §75):
//! - `oracle verify`        — verify the frozen archive hash against the lock
//! - `oracle info`          — print the frozen authority record
//! - `upstream diff <ref>`  — diff the locked revision against a candidate ref
//! - `court list`           — list all court case directories
//! - `court run <domain>/<case>` — fingerprint + compare a case (pure courts)
//! - `court run --all`      — run every court whose fixture is present
//!
//! VM-mediated courts (`cargo xtask vm build`, differential execution
//! against the real oracle) are Phase 2 and will extend this binary.

use std::path::Path;
use std::process::ExitCode;

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR"); // xtask/..
fn repo_root() -> &'static Path {
    Path::new(REPO_ROOT)
        .parent()
        .expect("xtask is a direct member")
}

fn lock_path() -> std::path::PathBuf {
    repo_root().join("oracle/UPSTREAM.lock")
}

fn main() -> ExitCode {
    let owned: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    match args.as_slice() {
        [cmd, rest @ ..] if *cmd == "oracle" => match rest {
            ["verify"] => oracle_verify(),
            ["info"] => oracle_info(),
            ["archive"] => oracle_archive(),
            other => {
                eprintln!("xtask oracle: unknown subcommand {other:?} (expected: verify | info | archive)");
                ExitCode::FAILURE
            }
        },
        ["upstream", "diff", reference] => upstream_diff(reference),
        ["court", "list"] => court_list(),
        ["court", "run", "--all"] => court_run_all(),
        ["court", "run", case] => court_run(case),
        _ => {
            eprintln!(
                "usage: cargo xtask <oracle verify|oracle info|oracle archive|upstream diff <ref>|court list|court run <case>|court run --all>"
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
