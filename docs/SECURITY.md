# SECURITY

Security posture and audit plan. Rust removes memory-safety classes; it does
not remove command-injection or privilege-design mistakes, which are the
primary risk surfaces here.

## High-risk surfaces (ranked)

1. **Root shell via polkit** — oracle `rootshell.sh` = `exec /bin/bash "$@"`.
   Mitigation: narrow typed privileged helper (docs/PRIVILEGE_MODEL.md);
   argv exec of pacman; no shell interpolation; validated package names;
   minimized environment; documented as SECURITY_CORRECTION.
2. **Shell construction** — oracle uses `popen(cmd, "r")` for probes and
   builds terminal command strings. Candidate: all probe commands are
   fixed-string templates with *validated* inputs only; the exec adapter
   executes argv directly where bash semantics are not required. PKGBUILD
   evaluation is delegated to bash deliberately (bash is the contract) inside
   a narrow adapter that captures argv/cwd/env/stdin/stdout/stderr/status.
3. **Custom pkgbase injection** — `pkgbase="<custom_name>"` is spliced into a
   PKGBUILD that bash then sources. A malicious custom name is a bash
   injection *if* the value can contain quotes/newlines. Oracle does no
   validation (user-typed into a QLineEdit). Candidate: reject bytes that
   would break the quoted-string splice (`"`, `\`, newline, CR, NUL) —
   documented as SECURITY_CORRECTION; valid upstream semantics (ASCII,
   hyphens, dots, `$pkgbase` and `$pkgbase-<suffix>` patterns) preserved.
4. **Patch list** — items are quoted `"..."` into `source=(...)`. Local
   patches carry a `file://` prefix; remote entries are user-typed URLs.
   Candidate: reject `"`, `\`, newline in patch entries (same splice
   boundary) and validate the `file://`/URL shape.
5. **Non-atomic PKGBUILD/config writes** — oracle truncates in place
   (`ofstream`, `File::create`). Candidate: write temp sibling + fsync +
   atomic rename, retaining before/after evidence (INTENTIONAL_CORRECTION,
   crash-resilience courts).
6. **Cache-directory trust** — `~/.cache/cachyos-km/pkgbuilds` is writable by
   the user and later executed (makepkg) and partially *as root* (nothing in
   the oracle's chain executes pkgbuilds as root — pacman -S/-Rsn only take
   package names; `sudo pacman -U` takes globs of *built artifacts* in the
   user's build dir). Threat model: the user's own cache is the user's own
   code; the boundary that matters is that **privileged operations never
   interpret cache contents**. Candidate keeps that invariant and documents
   it.
7. **Symlink/TOCTOU on probe scripts** (`.testscript*`, `.done-status`) —
   oracle writes into the repo/cwd dirs with fixed names. Candidate: use
   secure temp names / O_EXCL where the oracle's fixed names are not an
   external contract, and verify ownership/permissions before reuse.
8. **Environment inheritance** — oracle sets `_*` build vars via setenv into
   its own process; a leaked `BASH_ENV`/`ENV` would be sourced by the bash
   probes. Candidate execs probes with a scrubbed environment.
9. **ALPM FFI** — the only `unsafe` in the workspace; isolated, documented,
   invariant-covered, minimized (`#![forbid(unsafe_code)]` everywhere else).
10. **Single-instance lock** — QSharedMemory semantics must not become a
    denial vector for a legitimate second session; court the oracle's
    attach/detach retry behavior and stale-lock handling.

## Audit checklist (Phase 12) — dispositions

Each item below maps to a fix + regression court, or a documented
SECURITY_CORRECTION (with its divergence record). Status as of the Phase 12
hostile-review pass (2026-08-23; the two NEW courts that drive the packaged
binaries are `ui/gui-drive` — sort/toggle identity + machine residual — and
`ui/i18n-rendered` — the rendered translation surface under generated
locales).

| attack surface | disposition | witness |
|---|---|---|
| shell injection — custom pkgbase | D-003 grammar: `splice_safe_custom_name` permits the oracle's real defaults (`$pkgbase-custom`, `$pkgbase`) + a conservative literal grammar, REJECTS `"`, `\`, newline/CR/NUL, `${...}`, `$(...)`, backticks (SECURITY_CORRECTION; the frozen Qt app splices raw) | unit tests (incl. the default value), `custom-name/pkgbase-injection` (VM, real Configure-window mutation) |
| shell injection — remote patch URLs | D-003: `splice_unsafe_index` rejects the splice-breaking bytes in every `source=()` entry (SECURITY_CORRECTION) | unit tests, `patch-injection/source-array` (VM) |
| shell injection — local patch paths | same splice boundary; the `file://`-prefixed probe + empty-submission guards (audit P2) | unit tests, `patch-injection/source-array` |
| package names | come from libalpm (never user text); AUR names carry the conservative package-name grammar | `transaction-plan/*`, `kernel-discovery/*` courts |
| git URLs | `execute_git_cache_plan` runs git via argv (no shell); stale non-git checkouts wiped; `~` expanded (audit P1) | `git-cache/lifecycle` (VM, real Configure git chain) |
| temp files | unique per-call names + `O_EXCL` + mode 0600 in `exec_probe`/`run_probe` (audit P1/security — the old `File::create` followed a pre-created symlink) | probe-temp-file unit tests |
| symlink attacks | the O_EXCL fix above; `.done-status` lifecycle: removed at the exact oracle transition point (a stale marker can no longer classify a failed build as successful) | build contract tests |
| TOCTOU | mutable local-state facts (local db, module-family) re-probed at the transaction-planning boundary; only startup-static facts cached (audit P1) | `refresh_mutable_hardware` unit tests |
| inherited environment | per-child `Command::envs`; the process-global `setenv`/`restore_clean_environment` removed (D-010 — Rust's own docs: no process-global env mutation in multithreaded programs) | env unit tests |
| PATH resolution | fixed executable paths in the exec adapter; `terminal-helper` argv courted | `terminal-helper/emulator-matrix` |
| arbitrary command execution | `run_cmd_terminal` uses the packaged terminal-helper contract with fixed command templates; the privileged surface is being narrowed to a typed helper (D-001, Phase 13) | `privilege/rootshell-argv`, `terminal-helper/*` |
| command quoting | argv-based exec where bash semantics are not required; the PKGBUILD build is delegated to bash deliberately (bash is the contract) | exec adapter tests, `build-env/makepkg-runtime` |
| privilege escalation / polkit boundary | the polkit action identity + annotated exec path are courted (Phase 5); the production replacement is the narrow typed helper + polkit shim (D-001, Phase 13) | `privilege/*` courts |
| D-Bus trust | the scx client talks only to `org.scx.Loader` (system bus); the interface is a courted subset | `scx/loader-interface` (VM, real loader introspect) |
| writable cache directories | the invariant that privileged operations never interpret cache contents is documented (the user's own cache is the user's own code) | surface note above; `build-env/makepkg-runtime` |
| package artifact globs | `BuildFlowPlan::artifact_globs` renders the pacman `-U` globs from the variant's real artifact names | `artifact-glob/*` courts |
| stale artifacts | `.done-status` removed at the oracle transition point; the success→failed-next-build regression is unit-tested | build-guard tests |
| unsafe FFI | the alpm FFI is the ONLY `unsafe` in the workspace; every extern signature + layout is machine-verified at build time | `abi/probe.c` + `alpm-ffi/abi-surface` |
| integer/path conversion | the FFI's signed/unsigned + pointer conversions are audited and ABI-courted | `alpm-ffi/*` |
| Unicode/path confusion | the locale resolution (gap-009: `zh_CN` vs the `zh-CN` qrc alias) is courted, and the RENDERED projection under generated locales (de_DE + zh_CN) is witnessed on the packaged binaries | `ui/i18n-resolution`, `ui/i18n-rendered` (VM) |

Unresolved / characterized-only items:

- **gap-010** (close-during-transaction worker race, km-window.cpp:327-338):
  the oracle releases the alpm handle without joining the worker.
  REPRODUCED + courted 2026-08-23 by `ui/close-during-transaction` (fixture
  `close-transaction`, slow-pacman in-flight window): the frozen Qt app
  ABORTS on close-during-transaction (SIGABRT — Qt's "QThread: Destroyed
  while thread is still running" after the closeEvent's alpm_release lets
  the app exit while the worker is blocked), the release Slint binary
  exits CLEANLY (runtime-owned task; D-008), and the machine residuals
  match byte-for-byte.
