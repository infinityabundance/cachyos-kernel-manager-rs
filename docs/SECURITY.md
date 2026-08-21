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

## Audit checklist (Phase 12)

shell injection, custom names, patch URLs, local patch paths, package names,
git URLs, temp files, symlink attacks, TOCTOU, inherited environment, PATH
resolution, arbitrary command execution, command quoting, privilege
escalation, polkit boundary, D-Bus trust, writable cache directories,
package artifact globs, stale artifacts, unsafe FFI, integer/path
conversion, Unicode/path confusion.

Each finding produces either a fix + regression court, or a documented
SECURITY_CORRECTION with full divergence record.
