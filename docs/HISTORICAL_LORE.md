# HISTORICAL LORE

Lore mined from the upstream git history (474 commits, 2022-01-15 →
2026-08-21). Classifications follow the FRF taxonomy:
`CURRENT_CONTRACT`, `HISTORICAL_CONTRACT`, `FIXED_REGRESSION`,
`KNOWN_QUIRK`, `KNOWN_BUG`, `UNDERSPECIFIED`, `ENVIRONMENT_DEPENDENT`,
`DEAD_BEHAVIOR`.

## Origins

- `f897e30` (2022-01-15) initial commit; `a44eb56` (2022-01-23) v0.9.0.
- v0.9.0 moved all package work onto libalpm (`59e952e`), added a progress
  bar (`8411b13`), and — per the changelog — runs heavy work on a separate
  thread, updates kernels when installed-but-outdated, and prints a backtrace
  on crash.
- `23c76a5` (2022-01-26) "make it work without root privileges" — the
  application itself runs as the user; privilege is acquired per-operation.
- `b5bf22a` + `fb3742c` (2022-01-28) changed the execute-button/thread logic
  so the button can be used multiple times (changelog: "Change the behaviour
  of execute button and thread logic") — this is the current worker-thread +
  condition-variable design.

## NVIDIA lineage

| commit | change | classification |
|---|---|---|
| `47ed9b8` | add building with nvidia module | HISTORICAL_CONTRACT |
| `59f3bc7` | add nvidia kernel module if available and supported | CURRENT_CONTRACT (seed) |
| `6b69b50` | add zfs kernel module on ZFS if available | CURRENT_CONTRACT (seed) |
| `7446ae0` | properly remove zfs and nvidia modules | CURRENT_CONTRACT (removal companions) |
| `f1b72f2` | conf: add build open nvidia module option | CURRENT_CONTRACT (`builtin_nvidia_open`) |
| `e2aa183` | kernel: handle nvidia-open modules | CURRENT_CONTRACT |
| `12da796` | kernel: reset nvidia modules select if any of them already installed | CURRENT_CONTRACT (installed-module precedence) |
| `ff5f9f1` | kernel: try to install arch prebuild nvidia modules if available | CURRENT_CONTRACT (chwd + prebuilt) |
| `6d91ba2` (2026-03-13, Peter Jung) | Remove `build_nvidia` and update default kernel name | FIXED_REGRESSION — `_build_nvidia` env var removed; deprecated since NVIDIA 590 driver; also renamed default scheduler label to "tuned EEVDF" |

## ZFS lineage

- `6b69b50` seed; `bb7b016` "useless to reset patches on nvidia & zfs";
  `154a360` nvidia check should keep track of patches state; current behavior:
  only `builtin_nvidia_open_check` toggling refreshes the patches tab.
- ZFS root detection via `findmnt` has been stable since early versions.

## AUR lineage

- `62a1c7f` add AUR kernels support; `d98d9c0` disabled by default;
  `6c302e8` C++23 ranges; `5b075dc` (2026-06-23) **flip** the check from
  `!paru && !awk` to `!paru || !awk` — either one missing now disables AUR
  support. Also reworded the stderr message ("Paru & AWK are not installed!"
  → "Paru and/or AWK are not installed!"). Both sides of this flip are
  observable; the current contract is the `||` form.

## Terminal-helper lineage

- Garuda-derived ("temporal implementation" per the file header).
- `c90bf6c` refactor terminals; `721c29a` ptyxis; `16ed94c` rio (#33);
  `2dcf086` gnome-console/kgx (#49); `4b3a031` ghostty (#56).
- The `kgx` SIGQUIT hack exists because gnome-console does not exit when the
  command completes.
- KNOWN_QUIRK: the final line `eval ... 2>/dev/null || [[ "$terminal" !=
  "kgx" ]] && { rm "$file"; exit 2; }` — bash precedence `(A || B) && C` means
  the helper removes the temp file and exits 2 even after a *successful*
  terminal session (except when kgx itself failed). Callers must not treat
  exit code 2 as failure (the build flow keys on `.done-status`; the main
  window ignores the helper exit code entirely).
- The header comment's invariant "1. $TERMINAL must come first" is
  DEAD_BEHAVIOR — `$TERMINAL` is never read.

## Rootshell lineage

- `d919d39` "rootshell: hardcode bash path" — the helper is
  `exec /bin/bash "$@"`; polkit annotates
  `/usr/lib/cachyos-kernel-manager/rootshell.sh` with `auth_admin`.
- SECURITY_CORRECTION_CANDIDATE: arbitrary-root-shell via polkit is the
  current contract but is broader than the app needs.

## SCX lineage

- `515905e`/`d24dc50`/`eb7a386`/`f3eeaf6`/`cc79698` moved the sched-ext UI
  into a shared library, then out of this repository entirely
  (scxctl-ui). D-Bus calls moved into Rust (`425681d`, `c866d99`); flags are
  shown when changing profile/scheduler (`a147d57`, #30); Server mode
  support (`015a86d`); running mode/scheduler shown as initial values
  (`c1e0525`); args used only when they differ from defaults (`b70b01b`).

## Config subsystem lineage

- `6d91ba2` removed `build_nvidia` from `compile_options.json` and the Rust
  config struct; translations updated in lockstep (ca, cs, de, ko, nl, pl,
  ru, sk, sv, tr).
- The TOML schema is Rust-defined (serde) and is the public config surface;
  the C++ side never serializes config itself.

## Known quirks inventory (each needs a court)

1. `terminal-helper` exit-2-after-success (above).
2. `cachyos-kernel-manager_uk.ts` exists but is absent from
   `cachyoskm_locale.qrc` — Ukrainian is never loaded.
3. meson build references undeclared `pkg_dummy_impl` option and omits
   `config-option-lib`, `alpm_utils.cpp`, `config-options.cpp` from its
   sources — meson appears stale/broken relative to CMake.
4. `prepare_git_repo` mutates the process cwd and never restores it; the
   Configure build path derives from the *current* cwd (`fs::current_path()`
   at build time), which after the first configure is the pkgbuilds repo.
5. `$TERMINAL` documented but ignored by terminal-helper.
6. Version markers `∨`/`∧` are rendered via `fmt::format(FMT_COMPILE("∨{}"))`
   — literal U+2228/U+2227 characters in the source.
7. `fix_path` indexes `path[0]` unconditionally (empty string → UB) —
   callers never pass empty; candidate should not reproduce UB.
8. `restore_clean_environment` indexes `expr_split[1]` after splitting on `=`
   — a set-values line without `=` is UB; generated lines always contain `=`.
9. `utils::exec` returns the literal string `-1` when `popen` fails — the
   ZFS check compares that string against `zfs` (false) and the NVIDIA check
   treats it as a profile-name payload (a profile literally named `-1` would
   not match the `nvidia-*` prefixes).
10. Worker thread prints `Waiting... ` to stderr on every wake.
11. `on_execute` starts the worker thread via `m_worker_th->start()` on every
    execute (thread is already running; QThread::start on a running thread is
    a no-op warning).
12. Close during a running transaction: `closeEvent` releases ALPM and exits
    without joining the worker — a race the candidate must handle safely.
