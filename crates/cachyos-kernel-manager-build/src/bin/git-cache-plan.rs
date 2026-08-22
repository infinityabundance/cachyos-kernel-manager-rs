//! `git-cache-plan` — candidate model witness for the `git-cache/lifecycle`
//! court (directive §44).
//!
//! Probes the filesystem exactly as the oracle's `prepare_git_repo`
//! (`utils.cpp:161-196`) branches on — `fs::exists(parent_dir)`,
//! `fs::exists(repo_path)`, `fs::exists(repo_path/.git)` — computes the
//! [`git_cache_plan`] and prints the modeled exec chain in the transaction
//! witness schema (`cachyos-km-oracle-transaction-v1`), so the existing
//! transaction comparator compares the modeled git argv chain against the
//! oracle's strace-witnessed git argv chain.
//!
//! Usage: git-cache-plan <parent_dir> <repo_path> <clone_url>

use cachyos_kernel_manager_build::{git_cache_plan, GitCacheState, GitCacheStep};
use serde_json::json;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [parent, repo, url] = args.as_slice() else {
        eprintln!("usage: git-cache-plan <parent_dir> <repo_path> <clone_url>");
        return ExitCode::from(2);
    };
    let parent = Path::new(parent);
    let repo = Path::new(repo);
    let state = GitCacheState {
        parent_dir_exists: parent.exists(),
        repo_exists: repo.exists(),
        repo_is_git: repo.join(".git").exists(),
    };
    let plan = git_cache_plan(&state, parent, repo, url);
    // Candidate plan schema (`cachyos-km-candidate-plan-v1`): commands are
    // plain argv arrays; the normalizer reads them from `commands`, not
    // `execs` (which is the ORACLE witness schema). `terminal` is null — no
    // terminal-helper runs in the Configure flow.
    let commands: Vec<Vec<String>> = plan
        .iter()
        .filter_map(|step| match step {
            GitCacheStep::GitClone { url, name } => Some(vec![
                "git".into(),
                "clone".into(),
                url.clone(),
                name.clone(),
            ]),
            GitCacheStep::GitCheckoutForceMaster => Some(vec![
                "git".into(),
                "checkout".into(),
                "--force".into(),
                "master".into(),
            ]),
            GitCacheStep::GitCleanFd => Some(vec!["git".into(), "clean".into(), "-fd".into()]),
            GitCacheStep::GitPull => Some(vec!["git".into(), "pull".into()]),
            // directory/cwd steps are not execve witnesses
            _ => None,
        })
        .collect();
    let payload = json!({
        "schema": "cachyos-km-candidate-plan-v1",
        "probes": [],
        "commands": commands,
        "terminal": null,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
