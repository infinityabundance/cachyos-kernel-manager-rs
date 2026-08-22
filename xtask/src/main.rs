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

use cachyos_kernel_manager_frf::Residual;
use std::path::Path;
use std::process::{Command, ExitCode};

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR"); // xtask/..
fn repo_root() -> &'static Path {
    Path::new(REPO_ROOT)
        .parent()
        .expect("xtask is a direct member")
}

/// Quote a string for a single-quoted shell argument passed over ssh exec
/// (the in-VM scripts parse `--custom-name <v>` style args). Single quotes
/// are escaped the standard way (`'` -> `'\''`).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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
        ["scx", "verify"] => scx_verify(),
        ["upstream", "diff", reference] => upstream_diff(reference),
        ["court", "list"] => court_list(),
        ["court", "list", "--vm-capable"] => court_list(),
        ["court", "run", "--all"] => court_run_all(),
        ["court", "run", case, "--vm"] => court_run_vm(case),
        ["court", "run", case] => court_run(case),
        ["vm", "build"] => vm_build(),
        ["vm", "bake", fixture] => vm_bake(fixture),
        ["evidence", "verify"] => evidence_verify(),
        ["evidence", "release", name] => evidence_release(name),
        ["evidence", "verify-release", name] => evidence_verify_release(name),
        _ => {
            eprintln!(
                "usage: cargo xtask <oracle verify|info|archive|pkg-hash> | scx verify | upstream diff <ref> | court list | court run <case> [--vm] | court run --all | vm build | vm bake <fixture> | evidence verify | evidence release <name> | evidence verify-release <name>"
            );
            ExitCode::FAILURE
        }
    }
}

/// `scx verify` — verify both SCX authority archives (the pre-extraction
/// scx-manager UI archive + the scx_loader crate) against the lock (Phase 7).
fn scx_verify() -> ExitCode {
    let lock = match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot load lock: {e}");
            return ExitCode::FAILURE;
        }
    };
    match lock.verify_scx_archives(repo_root()) {
        Ok(results) if !results.is_empty() && results.iter().all(|(_, ok)| *ok) => {
            for (path, _) in &results {
                println!("scx authority OK: {path}");
            }
            ExitCode::SUCCESS
        }
        Ok(results) if results.is_empty() => {
            eprintln!("lock has no [scx] section");
            ExitCode::FAILURE
        }
        Ok(results) => {
            for (path, ok) in &results {
                if !ok {
                    eprintln!("scx authority MISMATCH: {path}");
                }
            }
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("scx verify error: {e}");
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
    let mut vm_only = false;
    if std::env::args().any(|a| a == "--vm-capable") {
        vm_only = true;
    }
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
            if !case.path().join("claim.toml").exists() {
                continue;
            }
            let id = format!(
                "{}/{}",
                domain.file_name().to_string_lossy(),
                case.file_name().to_string_lossy()
            );
            if vm_only {
                // a court is VM-capable iff its comparator declares a VM
                // mode via the actual TOML KEYS (fixture/transaction/
                // terminal_matrix/configure/mutate/scx) — comments like
                // "no baked fixture image" must NOT count.
                let Ok(comparator) = std::fs::read_to_string(case.path().join("comparator.toml"))
                else {
                    continue;
                };
                let vm_marker = [
                    "fixture = ",
                    "[transaction]",
                    "terminal_matrix = ",
                    "configure = ",
                    "[mutate]",
                    "scx = ",
                ]
                .iter()
                .any(|m| comparator.contains(m));
                if !vm_marker {
                    continue;
                }
            }
            println!("{id}");
            found += 1;
        }
    }
    println!(
        "{found} courts {}",
        if vm_only { "VM-capable" } else { "defined" }
    );
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
            // record the evidence for the publication layer (same record
            // shape the VM runner writes)
            let mut ev = cachyos_kernel_manager_casefile::evidence::EvidenceRecord {
                court: case_id.to_string(),
                oracle_revision: cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path())
                    .map(|l| l.oracle.commit)
                    .unwrap_or_else(|_| "unknown".to_string()),
                candidate_revision: env!("CARGO_PKG_VERSION").to_string(),
                fixture_digest: None,
                normalizers: vec![("stdout-capture".to_string(), "1.0.0".to_string())],
                comparator_version: "1.0.0".to_string(),
                result: "pass".to_string(),
                residual_count: 0,
                artifacts: vec![],
            };
            let _ = ev.add_directory(&case.dir.join("oracle"), "oracle");
            let _ = ev.add_directory(&case.dir.join("candidate"), "candidate");
            let _ = ev.write(&case.dir);
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
    // Phase 5: the plan tool (transaction courts)
    let plan_src = repo_root().join("target/debug/cachyos-kernel-manager-plan");
    if !plan_src.exists() {
        eprintln!("build the plan tool first: cargo build -p cachyos-kernel-manager-alpm --features libalpm --bin cachyos-kernel-manager-plan");
        return ExitCode::FAILURE;
    }
    std::fs::copy(&plan_src, share.join("inspect/cachyos-kernel-manager-plan")).expect("copy plan");
    // Phase 6: the git-cache model tool (configure-flow courts)
    let gitcache_src = repo_root().join("target/debug/cachyos-kernel-manager-gitcache");
    if !gitcache_src.exists() {
        eprintln!("build the git-cache model tool first: cargo build -p cachyos-kernel-manager-build --bin cachyos-kernel-manager-gitcache");
        return ExitCode::FAILURE;
    }
    std::fs::copy(
        &gitcache_src,
        share.join("inspect/cachyos-kernel-manager-gitcache"),
    )
    .expect("copy gitcache");
    // Phase 6: the mutation model tool (patch-injection/custom-name courts)
    let mutate_src = repo_root().join("target/debug/cachyos-kernel-manager-mutate");
    if !mutate_src.exists() {
        eprintln!("build the mutation model tool first: cargo build -p cachyos-kernel-manager-build --bin cachyos-kernel-manager-mutate");
        return ExitCode::FAILURE;
    }
    std::fs::copy(
        &mutate_src,
        share.join("inspect/cachyos-kernel-manager-mutate"),
    )
    .expect("copy mutate");
    // Phase 7: the scx client + interface tools (scx/loader-interface --vm)
    let scx_state_src = repo_root().join("target/debug/cachyos-kernel-manager-scx-state");
    let scx_iface_src = repo_root().join("target/debug/cachyos-kernel-manager-scx-introspect");
    if !scx_state_src.exists() || !scx_iface_src.exists() {
        eprintln!("build the scx tools first: cargo build -p cachyos-kernel-manager-scx --features dbus --bins");
        return ExitCode::FAILURE;
    }
    std::fs::copy(
        &scx_state_src,
        share.join("inspect/cachyos-kernel-manager-scx-state"),
    )
    .expect("copy scx-state");
    std::fs::copy(
        &scx_iface_src,
        share.join("inspect/cachyos-kernel-manager-scx-introspect"),
    )
    .expect("copy scx-introspect");
    // iterate without rebaking: fresh in-VM scripts via the 9p share
    let scripts_dst = share.join("scripts");
    let _ = std::fs::remove_dir_all(&scripts_dst);
    std::fs::create_dir_all(&scripts_dst).expect("share/scripts");
    for entry in std::fs::read_dir(repo_root().join("vm/in-vm")).expect("vm/in-vm") {
        let entry = entry.expect("entry");
        std::fs::copy(entry.path(), scripts_dst.join(entry.file_name()))
            .expect("copy in-vm script");
    }

    // Phase 5: transaction courts drive a real GUI transaction on the oracle
    // side; the scripts differ from the observe-only courts. The
    // terminal-matrix court runs the helper script against emulator stubs
    // on both sides.
    let is_transaction = case.comparator.transaction.is_some();
    let is_terminal_matrix = case.comparator.terminal_matrix.is_some();
    let is_configure = case.comparator.configure;
    let is_mutation = case.comparator.mutate.is_some();
    let is_scx = case.comparator.scx;
    let tx_select: Vec<String> = case
        .comparator
        .transaction
        .as_ref()
        .map(|t| t.select.clone())
        .unwrap_or_default();
    let mutate_spec = case.comparator.mutate.clone();

    // the packaged terminal-helper (candidate side of the matrix court)
    let packaged_helper =
        repo_root().join("packaging/usr/lib/cachyos-kernel-manager/terminal-helper");
    if is_terminal_matrix {
        if !packaged_helper.exists() {
            eprintln!("terminal-matrix court requires packaging/usr/lib/cachyos-kernel-manager/terminal-helper");
            return ExitCode::FAILURE;
        }
        std::fs::create_dir_all(share.join("packaging")).expect("share/packaging");
        std::fs::copy(&packaged_helper, share.join("packaging/terminal-helper"))
            .expect("copy packaged terminal-helper");
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
            if is_terminal_matrix {
                match side {
                    "oracle" => run(
                        "bash",
                        &[
                            ctl,
                            "exec",
                            "bash /mnt/host/scripts/terminal-matrix-run.sh /usr/lib/cachyos-kernel-manager/terminal-helper /mnt/host/out",
                        ],
                    )?,
                    "candidate" => run(
                        "bash",
                        &[
                            ctl,
                            "exec",
                            "bash /mnt/host/scripts/terminal-matrix-run.sh /mnt/host/packaging/terminal-helper /mnt/host/out",
                        ],
                    )?,
                    _ => unreachable!(),
                }
                return Ok(());
            }
            match side {
                "oracle" => {
                    let script = if is_scx {
                        "scx-loader-observe.sh"
                    } else if is_mutation {
                        "oracle-mutate.sh"
                    } else if is_configure {
                        "oracle-configure.sh"
                    } else if is_transaction {
                        "oracle-transact.sh"
                    } else {
                        "oracle-observe.sh"
                    };
                    let mut cmd = format!("bash /mnt/host/scripts/{script} /mnt/host/out");
                    if let Some(spec) = &mutate_spec {
                        cmd.push_str(&format!(
                            " --custom-name {} --patch-url {}",
                            shell_quote(&spec.custom_name),
                            shell_quote(&spec.patch_url)
                        ));
                    }
                    for raw in &tx_select {
                        cmd.push(' ');
                        cmd.push_str(raw);
                    }
                    run("bash", &[ctl, "exec", &cmd])?;
                }
                "candidate" => {
                    let script = if is_scx {
                        "scx-loader-candidate.sh"
                    } else if is_mutation {
                        "candidate-mutate.sh"
                    } else if is_configure {
                        "candidate-gitcache.sh"
                    } else if is_transaction {
                        "candidate-plan.sh"
                    } else {
                        "candidate-observe.sh"
                    };
                    let mut cmd = format!("bash /mnt/host/scripts/{script} /mnt/host/out");
                    if let Some(spec) = &mutate_spec {
                        cmd.push_str(&format!(
                            " --custom-name {} --patch-url {}",
                            shell_quote(&spec.custom_name),
                            shell_quote(&spec.patch_url)
                        ));
                    }
                    for raw in &tx_select {
                        cmd.push(' ');
                        cmd.push_str(raw);
                    }
                    run("bash", &[ctl, "exec", &cmd])?;
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
    let mut residuals: Vec<Residual> = Vec::new();
    if is_scx {
        match cachyos_kernel_manager_casefile::vm_court::compare_scx_interface(&case.dir, case_id) {
            Ok(mut r) => residuals.append(&mut r),
            Err(e) => {
                eprintln!("scx comparison error: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else if !is_terminal_matrix {
        residuals = match cachyos_kernel_manager_casefile::vm_court::compare_vm_observations(
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
    }
    if is_terminal_matrix {
        match cachyos_kernel_manager_casefile::vm_court::compare_terminal_matrix(&case.dir, case_id)
        {
            Ok(mut tx) => residuals.append(&mut tx),
            Err(e) => {
                eprintln!("terminal-matrix comparison error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if is_transaction || is_configure {
        match cachyos_kernel_manager_casefile::vm_court::compare_vm_transactions(&case.dir, case_id)
        {
            Ok(mut tx) => residuals.append(&mut tx),
            Err(e) => {
                eprintln!("transaction comparison error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if is_mutation {
        match cachyos_kernel_manager_casefile::vm_court::compare_mutation(&case.dir, case_id) {
            Ok(mut m) => residuals.append(&mut m),
            Err(e) => {
                eprintln!("mutation comparison error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

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

/// `evidence release <name>` — assemble an immutable evidence release
/// (directive §89, the publication layer): per-court recipe hashes,
/// content-addressed artifact hashes, normalizer/comparator versions,
/// fixture+image digests, FRF receipts, and the release root hash, written
/// to `evidence/releases/<name>/` (small hash files — committed). The raw
/// (gitignored) artifacts stay out of the repo; the hashes make any future
/// archive verifiable against this record.
fn evidence_release(name: &str) -> ExitCode {
    let lock = match cachyos_kernel_manager_oracle::UpstreamLock::load(&lock_path()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot load UPSTREAM.lock: {e}");
            return ExitCode::FAILURE;
        }
    };
    let git_commit = run_capture("git", &["rev-parse", "HEAD"])
        .unwrap_or_else(|e| format!("unknown ({e})"))
        .trim()
        .to_string();
    let created_at = run_capture("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();
    let builder = cachyos_kernel_manager_casefile::evidence_release::ReleaseBuilder {
        release: name.to_string(),
        created_at,
        git_commit: git_commit.clone(),
        oracle_revision: lock.oracle.commit.clone(),
        candidate_revision: env!("CARGO_PKG_VERSION").to_string(),
        base_image_hash: lock.oracle.reference_image_hash.clone(),
    };
    match builder.write_release(repo_root(), &repo_root().join("courts")) {
        Ok(dir) => {
            println!("evidence release {name} written to {}", dir.display());
            let root = std::fs::read_to_string(dir.join("ROOT-HASH")).unwrap_or_default();
            println!("root hash: {root}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("evidence release error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `evidence verify-release <name>` — verify a written release against the
/// current filesystem: artifact hashes (where the raw evidence is present),
/// the FRF receipt hashes, and the root hash.
fn evidence_verify_release(name: &str) -> ExitCode {
    match cachyos_kernel_manager_casefile::evidence_release::verify_release(
        repo_root(),
        &repo_root().join("courts"),
        name,
    ) {
        Ok(problems) if problems.is_empty() => {
            println!("evidence release {name}: VERIFIED (artifact hashes + root hash consistent)");
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            for p in &problems {
                eprintln!("release problem: {p}");
            }
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("release verify error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Run a command and capture stdout (trimmed), failing softly.
fn run_capture(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("{cmd} exited with {}", out.status))
    }
}
