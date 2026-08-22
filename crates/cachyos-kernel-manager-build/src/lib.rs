//! Build subsystem semantics, reconstructed from `oracle/upstream/src/
//! conf-window.cpp` (revision `6b4a373e`). Pure string/state logic so it can
//! be courted byte-for-byte against the oracle's mutations
//! (`courts/patch-injection/*`, `courts/custom-name/*`, `courts/build-env/*`).
//!
//! Everything here reproduces the oracle's *residuals*: the mutated PKGBUILD
//! text, the probe script text, the env string, the artifact globs.

#![forbid(unsafe_code)]

use cachyos_kernel_manager_core::options::BuildOptions;
use std::path::Path;

/// The build command rendered by the oracle (`conf-window.cpp:734`). The
/// success marker is `.done-status`, not the exit code.
pub const MAKEPKG_BUILD_COMMAND: &str =
    "makepkg -scf --cleanbuild --skipchecksums && touch .done-status";

/// The AUR build command (`aur_kernel.cpp:53`).
pub const MAKEPKG_AUR_COMMAND: &str = "makepkg -sicf --cleanbuild --skipchecksums";

/// Success marker file created after a successful build
/// (`conf-window.cpp:734,384-389`).
pub const DONE_STATUS: &str = ".done-status";

/// `get_source_array_from_pkgbuild` testscript (`conf-window.cpp:204-216`).
/// The options env string is spliced in verbatim; the script sources the
/// PKGBUILD and echoes the evaluated `source` array.
pub fn source_array_probe_script(options_env: &str) -> String {
    format!("#!/usr/bin/bash\n{options_env}source \"$1\"\necho \"${{source[@]}}\"")
}

/// `get_pkgext_value_from_makepkgconf` testscript (`conf-window.cpp:218-236`).
pub fn pkgext_probe_script() -> String {
    "#!/usr/bin/bash\nsource \"/etc/makepkg.conf\"\necho \"${PKGEXT}\"".to_string()
}

/// `get_package_names_glob_from_pkgbuild` testscript (`conf-window.cpp:274-298`).
pub fn pkgfuncs_probe_script() -> String {
    "#!/usr/bin/bash\nsource \"$1\"\ndeclare -F;echo \"pkgver: $pkgver-$pkgrel\"".to_string()
}

/// Build a `source=(...)` block exactly like `insert_new_source_array_
/// into_pkgbuild` (`conf-window.cpp:300-326`): original entries that do NOT
/// end with `.patch`, then the patch-list entries, each wrapped in `"..."`,
/// joined with `\n`, rendered `source=(\n...)\n`.
///
/// Quoting is the oracle's `fmt::format("\"{}\"", entry)` — no escaping.
/// The candidate validates entries before this point (see docs/SECURITY.md,
/// divergence D-003).
pub fn build_source_array(orig_entries: &[String], patches: &[String]) -> String {
    let mut entries: Vec<String> = Vec::new();
    for entry in orig_entries {
        if !entry.ends_with(".patch") {
            entries.push(format!("\"{entry}\""));
        }
    }
    for patch in patches {
        entries.push(format!("\"{patch}\""));
    }
    format!("source=(\n{})\n", entries.join("\n"))
}

/// The oracle's insertion primitive: find `marker` in `text`; find the last
/// `\n` at-or-before the marker position; insert `insertion` there.
///
/// **Oracle no-op semantics**: when the marker or the preceding newline is
/// missing, the oracle does NOT insert anything but still reports success
/// and rewrites the file unchanged (`conf-window.cpp:320-324,333-337` — the
/// `if (...)` guards skip the insert; `write_to_file` still runs). The build
/// proceeds without the mutation. This is reproduced faithfully: the
/// returned string is the (possibly unchanged) text, and the caller must
/// detect a no-op via [`inserted_before_marker`].
pub fn insert_before_marker(text: &str, marker: &str, insertion: &str) -> String {
    match insert_before_marker_changed(text, marker, insertion) {
        Some((out, _)) => out,
        None => text.to_string(),
    }
}

/// Like [`insert_before_marker`] but reports whether an insertion occurred.
pub fn insert_before_marker_changed(
    text: &str,
    marker: &str,
    insertion: &str,
) -> Option<(String, bool)> {
    let marker_pos = text.find(marker)?;
    let last_newline = text[..=marker_pos].rfind('\n')?;
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.push_str(&text[..last_newline]);
    out.push_str(insertion);
    out.push_str(&text[last_newline..]);
    Some((out, true))
}

/// Insert the user's patches into a PKGBUILD (`insert_new_source_array_
/// into_pkgbuild`). `orig_source_entries` is the evaluated source array from
/// the probe; `patches` is the patches-tab list. Returns the (possibly
/// unchanged) PKGBUILD text; the oracle reports success either way.
pub fn insert_patch_source_array(
    pkgbuild: &str,
    orig_source_entries: &[String],
    patches: &[String],
) -> String {
    let block = build_source_array(orig_source_entries, patches);
    insert_before_marker(pkgbuild, "prepare()", &block)
}

/// Insert the custom `pkgbase` (`set_custom_name_in_pkgbuild`,
/// `conf-window.cpp:328-339`): `\n\npkgbase="<custom>"` before the last
/// newline preceding `_major=`. Silent no-op (unchanged text) when
/// `_major=` is absent or at the start of the file — the oracle proceeds.
pub fn insert_custom_pkgbase(pkgbuild: &str, custom_name: &str) -> String {
    let insertion = format!("\n\npkgbase=\"{custom_name}\"");
    insert_before_marker(pkgbuild, "_major=", &insertion)
}

/// `prepare_func_names` + `get_package_names_glob_from_pkgbuild`
/// (`conf-window.cpp:238-298`): for each `package_<suffix>` function,
/// produce the glob `<suffix>-<pkgver>-<pkgrel>-*<PKGEXT>`.
pub fn artifact_globs(
    pkg_func_suffixes: &[String],
    pkgver: &str,
    pkgrel: &str,
    pkgext: &str,
) -> Vec<String> {
    pkg_func_suffixes
        .iter()
        .map(|suffix| format!("{suffix}-{pkgver}-{pkgrel}-*{pkgext}"))
        .collect()
}

/// Parse the `declare -F` + `pkgver:` output of the funcs probe.
/// Returns `(pkg_func_suffixes, Option<(pkgver, pkgrel)>)`; `None` for
/// pkgver means "broken pkgbuild; pkgver must be present"
/// (`conf-window.cpp:289-293`).
pub fn parse_pkgfuncs_probe_output(output: &str) -> (Vec<String>, Option<(String, String)>) {
    let mut suffixes = Vec::new();
    let mut pkgver = None;
    for line in output.lines() {
        if let Some(fn_name) = line.strip_prefix("declare -f ") {
            if let Some(suffix) = fn_name.strip_prefix("package_") {
                suffixes.push(suffix.to_string());
            }
        } else if let Some(v) = line.strip_prefix("pkgver: ") {
            if let Some((ver, rel)) = v.split_once('-') {
                pkgver = Some((ver.to_string(), rel.to_string()));
            }
        }
    }
    (suffixes, pkgver)
}

/// Split the `source` array probe output on spaces exactly like
/// `make_multiline(src_entries, ' ')` (`conf-window.cpp:215`): entries
/// separated by single spaces, empty entries dropped (the split-view filter
/// removes empty ranges).
pub fn parse_source_array_probe_output(output: &str) -> Vec<String> {
    output
        .split(' ')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// The full set of probe/build file names the oracle writes (fixed names,
/// reused across runs; a TOCTOU/symlink surface — see docs/SECURITY.md).
pub mod probe_files {
    pub const SOURCE_ARRAY: &str = ".testscript";
    pub const PKGEXT: &str = ".testscriptpkgext";
    pub const PKGFUNCS: &str = ".testscriptpkgnames";
    pub const DONE_STATUS: &str = ".done-status";
}

/// Render the complete env string consumed by probes (delegates to the core
/// options model; kept here so the build crate owns the boundary).
pub fn options_env_string(options: &BuildOptions) -> String {
    options.env_string()
}

/// Filesystem facts `prepare_git_repo` branches on (`utils.cpp:161-196`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitCacheState {
    /// `parent_dir` exists (`fs::create_directories` runs regardless).
    pub parent_dir_exists: bool,
    /// `repo_path` exists.
    pub repo_exists: bool,
    /// `repo_path/.git` exists (only meaningful when `repo_exists`).
    pub repo_is_git: bool,
}

/// One step of the oracle's `prepare_git_repo` sequence (`utils.cpp:161-196`).
/// The order matters: this is the exact argv/cwd chain the VM court
/// (`courts/git-cache/lifecycle`) witnesses via strace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitCacheStep {
    /// `fs::create_directories(parent_dir)` — errors swallowed (ec ignored).
    CreateDirectories,
    /// Enter (cwd := parent_dir); failure aborts the whole sequence.
    EnterParentDir,
    /// A non-git `repo_path` directory is wiped (`fs::remove_all`) before
    /// cloning — the "stale non-git dir" quirk.
    RemoveNonGitRepo,
    /// `git clone <url> <repo_name>` from cwd = parent_dir; a nonzero exit
    /// aborts the sequence.
    GitClone { url: String, name: String },
    /// Enter (cwd := repo_path); failure aborts the sequence.
    EnterRepoDir,
    /// `git checkout --force master`; failure short-circuits the refresh
    /// (no clean/pull) and only prints.
    GitCheckoutForceMaster,
    /// `git clean -fd`; failure short-circuits the refresh (no pull).
    GitCleanFd,
    /// `git pull`; failure only prints.
    GitPull,
}

/// The full command/cwd plan for `prepare_git_repo` (`utils.cpp:161-196`),
/// determined purely by the filesystem state. Abort points (EnterParentDir,
/// GitClone, EnterRepoDir) stop the execution at that step; the steps after
/// them are never attempted and the caller must not run them.
pub fn git_cache_plan(
    state: &GitCacheState,
    parent_dir: &Path,
    repo_path: &Path,
    clone_url: &str,
) -> Vec<GitCacheStep> {
    // The oracle re-checks `fs::exists` after `remove_all` (utils.cpp:177-185),
    // so a wiped non-git dir is followed by a clone. (`parent_dir` is only
    // entered, never executed — EnterParentDir is a cwd step, not an execve.)
    let _ = parent_dir;
    let mut plan = vec![
        GitCacheStep::CreateDirectories,
        GitCacheStep::EnterParentDir,
    ];
    let removed = state.repo_exists && !state.repo_is_git;
    if removed {
        plan.push(GitCacheStep::RemoveNonGitRepo);
    }
    if !state.repo_exists || removed {
        plan.push(GitCacheStep::GitClone {
            url: clone_url.to_string(),
            name: repo_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string(),
        });
    }
    plan.push(GitCacheStep::EnterRepoDir);
    plan.push(GitCacheStep::GitCheckoutForceMaster);
    plan.push(GitCacheStep::GitCleanFd);
    plan.push(GitCacheStep::GitPull);
    plan
}

/// `restore_clean_environment` (`utils.cpp:204-227`): unset every previously
/// set option (in order), then apply `all_set_values` line by line and
/// return the newly set variable names.
///
/// Returns `(unsets, sets)` — `unsets` are the previously set names, `sets`
/// are `(var, value)` parsed from `all_set_values`.
pub fn clean_env_plan(
    previously_set_options: &[String],
    all_set_values: &str,
) -> (Vec<String>, Vec<(String, String)>) {
    (
        previously_set_options.to_vec(),
        env_assignments(all_set_values),
    )
}

/// Parse `all_set_values` exactly like the oracle's `make_split_view` +
/// `make_multiline` split pair (`string_utils.hpp:36-71`): lines split on
/// `\n` with empty segments dropped, each line split on `=` with empty
/// segments dropped, `var = parts[0]`, `value = parts[1]`.
///
/// Oracle quirks reproduced: a value containing `=` is truncated at the
/// second `=` boundary (`expr_split[1]` only). A line with fewer than two
/// non-empty `=` segments makes the oracle read `expr_split[1]` out of
/// bounds (UB); the candidate skips such lines instead (D-005, parity does
/// not mean immortalizing defects).
pub fn env_assignments(all_set_values: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in all_set_values.split('\n') {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('=').filter(|s| !s.is_empty());
        match (parts.next(), parts.next()) {
            (Some(var), Some(value)) => {
                out.push((var.to_string(), value.to_string()));
            }
            // Oracle UB (`expr_split[1]` on a single-element vector).
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_scripts_match_oracle_text() {
        assert_eq!(
            source_array_probe_script("_cachy_config=yes\n"),
            "#!/usr/bin/bash\n_cachy_config=yes\nsource \"$1\"\necho \"${source[@]}\""
        );
        assert_eq!(
            pkgext_probe_script(),
            "#!/usr/bin/bash\nsource \"/etc/makepkg.conf\"\necho \"${PKGEXT}\""
        );
        assert_eq!(
            pkgfuncs_probe_script(),
            "#!/usr/bin/bash\nsource \"$1\"\ndeclare -F;echo \"pkgver: $pkgver-$pkgrel\""
        );
    }

    #[test]
    fn source_array_block_matches_oracle_format() {
        let orig = vec![
            "https://github.com/cachyos/linux-cachyos.git".to_string(),
            "some.patch".to_string(),
        ];
        let patches = vec!["file:///home/u/foo.patch".to_string()];
        let block = build_source_array(&orig, &patches);
        // Oracle format string is "source=(\n{})\n" — no newline between the
        // last entry and the closing paren.
        assert_eq!(
            block,
            "source=(\n\"https://github.com/cachyos/linux-cachyos.git\"\n\"file:///home/u/foo.patch\")\n"
        );
        // .patch entries from the ORIGINAL array are dropped, patch-list kept
    }

    #[test]
    fn patch_insertion_before_prepare() {
        let pkgbuild = "pkgbase=linux-cachyos\npkgver=6.14.1\n\nprepare() {\n  true\n}\n";
        let orig = vec!["https://x/linux.tar.gz".to_string()];
        let patches = vec!["file:///home/u/a.patch".to_string()];
        let out = insert_patch_source_array(pkgbuild, &orig, &patches);
        assert_eq!(
            out,
            "pkgbase=linux-cachyos\npkgver=6.14.1\nsource=(\n\"https://x/linux.tar.gz\"\n\"file:///home/u/a.patch\")\n\nprepare() {\n  true\n}\n"
        );
        // The original source=(...) block is NOT removed — the inserted one
        // precedes prepare() so the later assignment wins in bash.
        // Oracle byte-behavior note: the block lands at the last newline
        // before `prepare()` (here, the one ending the blank line).
    }

    #[test]
    fn patch_insertion_noop_without_prepare() {
        // Oracle: no prepare() -> no insert, but the build still proceeds
        // (silent no-op success).
        let pkgbuild = "pkgbase=linux-cachyos\npkgver=6.14.1\n";
        assert_eq!(insert_patch_source_array(pkgbuild, &[], &[]), pkgbuild);
    }

    #[test]
    fn pkgbase_insertion_before_major() {
        // `_major=` at the start of the file: no preceding newline -> oracle
        // no-op (unchanged text), still success.
        let pkgbuild = "_major=6\n_minor=14\n\nbuild() {\n true\n}\n";
        assert_eq!(insert_custom_pkgbase(pkgbuild, "$pkgbase-custom"), pkgbuild);
    }

    #[test]
    fn pkgbase_insertion_with_header() {
        let pkgbuild = "# Maintainer: x\n_major=6\n";
        let out = insert_custom_pkgbase(pkgbuild, "my-kernel");
        // insertion = "\n\npkgbase=\"my-kernel\"" lands at the newline ending
        // the header line; the original newline stays after the insertion.
        assert_eq!(out, "# Maintainer: x\n\npkgbase=\"my-kernel\"\n_major=6\n");
    }

    #[test]
    fn pkgbase_insertion_noop_without_major() {
        assert_eq!(insert_custom_pkgbase("pkgver=6\n", "x"), "pkgver=6\n");
    }

    #[test]
    fn artifact_globs_join_pkgver_pkgrel_and_pkgext() {
        // split packages: suffix = the `package_<suffix>` function names
        let globs = artifact_globs(
            &["linux-cachyos".into(), "linux-cachyos-headers".into()],
            "6.14.1",
            "3",
            ".pkg.tar.zst",
        );
        assert_eq!(
            globs,
            vec![
                "linux-cachyos-6.14.1-3-*.pkg.tar.zst",
                "linux-cachyos-headers-6.14.1-3-*.pkg.tar.zst"
            ]
        );
    }

    #[test]
    fn parse_funcs_probe_output() {
        // Realistic split-package function names from the linux-cachyos
        // PKGBUILD: `package_<pkgname>()` functions.
        let out = "declare -f package_linux-cachyos\ndeclare -f package_linux-cachyos-headers\ndeclare -f prepare\npkgver: 6.14.1-3\n";
        let (suffixes, pkgver) = parse_pkgfuncs_probe_output(out);
        assert_eq!(suffixes, vec!["linux-cachyos", "linux-cachyos-headers"]);
        assert_eq!(pkgver, Some(("6.14.1".to_string(), "3".to_string())));
        // 'declare -f prepare' does not start with package_ -> ignored
        // functions like `package` (no underscore) are dropped by the
        // oracle's `starts_with("package_")` filter
    }

    #[test]
    fn broken_pkgbuild_missing_pkgver() {
        let (_, pkgver) = parse_pkgfuncs_probe_output("declare -f package\n");
        assert!(pkgver.is_none());
    }

    #[test]
    fn source_array_probe_output_split_on_spaces() {
        let out = "https://a/x.tar.gz https://b/foo.patch";
        assert_eq!(
            parse_source_array_probe_output(out),
            vec!["https://a/x.tar.gz", "https://b/foo.patch"]
        );
        assert!(parse_source_array_probe_output("").is_empty());
    }

    #[test]
    fn build_command_contains_done_status_marker() {
        assert!(MAKEPKG_BUILD_COMMAND.contains("&& touch .done-status"));
    }

    #[test]
    fn git_cache_plan_fresh_clone() {
        let state = GitCacheState {
            parent_dir_exists: false,
            repo_exists: false,
            repo_is_git: false,
        };
        let plan = git_cache_plan(
            &state,
            Path::new("/home/u/.cache/cachyos-km"),
            Path::new("/home/u/.cache/cachyos-km/pkgbuilds"),
            "https://github.com/cachyos/linux-cachyos.git",
        );
        assert_eq!(
            plan,
            vec![
                GitCacheStep::CreateDirectories,
                GitCacheStep::EnterParentDir,
                GitCacheStep::GitClone {
                    url: "https://github.com/cachyos/linux-cachyos.git".into(),
                    name: "pkgbuilds".into(),
                },
                GitCacheStep::EnterRepoDir,
                GitCacheStep::GitCheckoutForceMaster,
                GitCacheStep::GitCleanFd,
                GitCacheStep::GitPull,
            ]
        );
    }

    #[test]
    fn git_cache_plan_existing_checkout_refreshes_without_clone() {
        let state = GitCacheState {
            parent_dir_exists: true,
            repo_exists: true,
            repo_is_git: true,
        };
        let plan = git_cache_plan(
            &state,
            Path::new("/home/u/.cache/cachyos-km"),
            Path::new("/home/u/.cache/cachyos-km/pkgbuilds"),
            "https://github.com/cachyos/linux-cachyos.git",
        );
        assert_eq!(
            plan,
            vec![
                GitCacheStep::CreateDirectories,
                GitCacheStep::EnterParentDir,
                GitCacheStep::EnterRepoDir,
                GitCacheStep::GitCheckoutForceMaster,
                GitCacheStep::GitCleanFd,
                GitCacheStep::GitPull,
            ]
        );
    }

    #[test]
    fn git_cache_plan_non_git_dir_wiped_then_cloned() {
        // The "stale non-git dir" quirk: repo exists but has no .git -> it
        // is removed, then the clone runs from the (still entered) parent.
        let state = GitCacheState {
            parent_dir_exists: true,
            repo_exists: true,
            repo_is_git: false,
        };
        let plan = git_cache_plan(
            &state,
            Path::new("/home/u/.cache/cachyos-km"),
            Path::new("/home/u/.cache/cachyos-km/pkgbuilds"),
            "https://github.com/cachyos/linux-cachyos.git",
        );
        assert_eq!(
            plan,
            vec![
                GitCacheStep::CreateDirectories,
                GitCacheStep::EnterParentDir,
                GitCacheStep::RemoveNonGitRepo,
                GitCacheStep::GitClone {
                    url: "https://github.com/cachyos/linux-cachyos.git".into(),
                    name: "pkgbuilds".into(),
                },
                GitCacheStep::EnterRepoDir,
                GitCacheStep::GitCheckoutForceMaster,
                GitCacheStep::GitCleanFd,
                GitCacheStep::GitPull,
            ]
        );
    }

    #[test]
    fn env_assignments_parse_lines_and_truncate_extra_equals() {
        // Empty lines are dropped; a value containing '=' is truncated at
        // the second '=' boundary (expr_split[1] only) — oracle quirk.
        let sets = env_assignments("_cc_harder=yes\n_HZ_ticks=1000\n\n_cpu=zen4=x\n");
        assert_eq!(
            sets,
            vec![
                ("_cc_harder".to_string(), "yes".to_string()),
                ("_HZ_ticks".to_string(), "1000".to_string()),
                ("_cpu".to_string(), "zen4".to_string()),
            ]
        );
    }

    #[test]
    fn env_assignments_skips_lines_without_separator() {
        // "=v", "a=" and bare lines make the oracle read expr_split[1] out
        // of bounds (UB); the candidate skips them (D-005).
        assert!(env_assignments("=v\na=\nbare\n").is_empty());
        assert!(env_assignments("").is_empty());
    }

    #[test]
    fn clean_env_plan_unsets_previous_then_applies() {
        let prev = vec!["_lto".to_string(), "_cc_harder".to_string()];
        let (unsets, sets) = clean_env_plan(&prev, "_lto=thin\n_cc_harder=yes\n");
        assert_eq!(unsets, prev);
        assert_eq!(
            sets,
            vec![
                ("_lto".to_string(), "thin".to_string()),
                ("_cc_harder".to_string(), "yes".to_string()),
            ]
        );
    }

    #[test]
    fn clean_env_plan_deduplicates_no_env_leakage() {
        // Variables set in the previous run are unset before the new ones
        // are applied; a var absent from the new values is not re-added.
        let prev = vec!["_stale_var".to_string()];
        let (unsets, sets) = clean_env_plan(&prev, "_lto=thin\n");
        assert_eq!(unsets, vec!["_stale_var".to_string()]);
        assert_eq!(sets, vec![("_lto".to_string(), "thin".to_string())]);
    }
}
