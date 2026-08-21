//! Candidate observation tool — run inside the court VM.
//!
//! Dumps the candidate's view of the package state through the SAME
//! surfaces the oracle observes:
//!
//! - sync databases registered from `/etc/pacman.conf` via the oracle's
//!   mINI parsing + registration rule (skip `testing`/`options`),
//! - kernel discovery rows (repo/name/headers/version/companions, same-db
//!   pairing) with the oracle's display semantics (raw name, category,
//!   version markers, update flag, installed provenance),
//! - the environment probes the oracle runs at startup (findmnt, chwd).
//!
//! Output schema `cachyos-km-candidate-state-v1` mirrors the observables of
//! `oracle-state.json` so the FRF comparator can align them.
//!
//! Usage:
//!   cachyos-kernel-manager-inspect dump [--json]
//!   cachyos-kernel-manager-inspect vercmp <a> <b>     (ALPM semantics)
//!   cachyos-kernel-manager-inspect pacman-conf        (mINI section dump)

use cachyos_kernel_manager_alpm::ffi::AlpmHandle;
use cachyos_kernel_manager_alpm::pacman_conf::{register_sections, MiniIni};
use cachyos_kernel_manager_core::discovery::{companions_for, CompanionNames, DiscoveredKernel};
use cachyos_kernel_manager_core::kernel::{
    classify_category, kernel_headers_name, matches_headers_needle, strip_version_marker,
    DisplayVersion,
};
use std::io::Read;

const PACMAN_CONF: &str = "/etc/pacman.conf";
const ALPM_ROOT: &str = "/";
const ALPM_DBPATH: &str = "/var/lib/pacman/";

fn main() {
    let owned: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    match args.as_slice() {
        [cmd, rest @ ..] if *cmd == "dump" => dump(rest.iter().any(|a| *a == "--json")),
        ["vercmp", a, b] => {
            let handle = open();
            println!("{}", handle.vercmp(a, b));
        }
        ["pacman-conf"] => {
            let content = read_pacman_conf();
            let ini = MiniIni::parse(&content);
            for s in register_sections(&ini) {
                println!("{s}");
            }
        }
        _ => {
            eprintln!("usage: inspect dump [--json] | vercmp <a> <b> | pacman-conf");
            std::process::exit(2);
        }
    }
}

fn open() -> AlpmHandle {
    AlpmHandle::init(ALPM_ROOT, ALPM_DBPATH).unwrap_or_else(|e| panic!("alpm init: {e}"))
}

fn read_pacman_conf() -> String {
    let mut s = String::new();
    std::fs::File::open(PACMAN_CONF)
        .unwrap_or_else(|e| panic!("{PACMAN_CONF}: {e}"))
        .read_to_string(&mut s)
        .expect("read pacman.conf");
    s
}

fn exec_stdout(cmd: &str, args: &[&str]) -> String {
    let out = std::process::Command::new(cmd).args(args).output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.trim_end_matches('\n').to_string()
        }
        Ok(o) => String::from_utf8_lossy(&o.stderr).into_owned(),
        Err(e) => format!("ERR:{e}"),
    }
}

/// Reconstruct the oracle's discovery (`kernel.cpp:179-286`) using the
/// candidate's libalpm handle. Row order = sync db registration order × db
/// package order (the needle-filter court verifies equivalence with the
/// oracle's `alpm_db_search` ordering).
fn discover(handle: &AlpmHandle) -> Vec<DiscoveredKernel> {
    let mut out = Vec::new();
    for db_name in handle.syncdb_names() {
        let packages = handle.db_packages(&db_name);
        let by_name = |name: &str| handle.db_get_pkg(&db_name, name);
        for pkg in &packages {
            if !matches_headers_needle(&pkg.name) || pkg.name.contains("linux-api-headers") {
                continue;
            }
            let headers = pkg.name.clone();
            let kernel_name = kernel_headers_name(&headers);
            let Some(kernel_pkg) = by_name(&kernel_name) else {
                continue;
            };
            let names = companions_for(&kernel_name);
            let companions = CompanionNames {
                zfs: names.zfs.filter(|n| by_name(n).is_some()),
                nvidia: names.nvidia.filter(|n| by_name(n).is_some()),
                nvidia_open: names.nvidia_open.filter(|n| by_name(n).is_some()),
            };
            out.push(DiscoveredKernel {
                repo: db_name.clone(),
                name: kernel_name.clone(),
                headers,
                version: kernel_pkg.version.clone(),
                companions,
                raw: format!("{db_name}/{kernel_name}"),
            });
        }
    }
    out
}

fn dump(pretty: bool) {
    let handle = open();
    let content = read_pacman_conf();
    let ini = MiniIni::parse(&content);
    let registered = register_sections(&ini);
    for name in &registered {
        handle.register_syncdb(name);
    }

    let kernels = discover(&handle);

    let mut rows = Vec::new();
    for k in &kernels {
        let local = handle.local_pkg(&k.name);
        // Oracle display semantics (`km-window.cpp:97-104`): a kernel
        // installed from a DIFFERENT sync repo is NOT skipped — the
        // QTreeWidgetItem was already inserted before the `continue`, so
        // the row REMAINS in the tree, unchecked and NOT immutable (the
        // `continue` only skips the immutable/checked marking). The
        // comparator sees the tree, so the candidate emits the same rows.
        let display = match &local {
            Some(l) => DisplayVersion::compute(Some(&l.version), &k.version, |a, b| {
                handle.vercmp(a, b).cmp(&0)
            }),
            None => DisplayVersion::compute(None, &k.version, |_, _| std::cmp::Ordering::Equal),
        };
        let immutable = local
            .as_ref()
            .map(|l| match &l.installed_db {
                None => true, // unknown provenance counts as matching (oracle)
                Some(db) => db == &k.repo,
            })
            .unwrap_or(false);
        rows.push(serde_json::json!({
            "raw": k.raw,
            "repo": k.repo,
            "name": k.name,
            "headers": k.headers,
            "sync_version": k.version,
            "installed": local.is_some(),
            "installed_db": local.as_ref().and_then(|l| l.installed_db.clone()),
            "immutable": immutable,
            "checked_default": local.is_some() && immutable,
            "display_version": display.text,
            "update_available": display.update,
            "category": classify_category(&k.name).display(),
            "sort_key_version": strip_version_marker(&display.text),
            "companions": {
                "zfs": k.companions.zfs,
                "nvidia": k.companions.nvidia,
                "nvidia_open": k.companions.nvidia_open,
            },
        }));
    }

    // environment probes the oracle runs at static init
    let zfs_root = exec_stdout("findmnt", &["-ln", "-o", "FSTYPE", "/"]) == "zfs";
    let chwd_out = exec_stdout(
        "bash",
        &[
            "-c",
            "chwd --list-installed -d 2>/dev/null | grep Name | awk '{print $4}'",
        ],
    );
    let chwd_nvidia = chwd_out.lines().any(|l| l.starts_with("nvidia-dkms"));
    let chwd_nvidia_open = chwd_out.lines().any(|l| l.starts_with("nvidia-open-dkms"));

    let payload = serde_json::json!({
        "schema": "cachyos-km-candidate-state-v1",
        "pacman_conf_sections": registered,
        "alpm_root": ALPM_ROOT,
        "alpm_dbpath": ALPM_DBPATH,
        "kernels": rows,
        "env": {
            "zfs_root": zfs_root,
            "chwd_nvidia": chwd_nvidia,
            "chwd_nvidia_open": chwd_nvidia_open,
        },
    });

    let out = if pretty {
        serde_json::to_string_pretty(&payload).expect("serialize")
    } else {
        serde_json::to_string(&payload).expect("serialize")
    };
    println!("{out}");
}
