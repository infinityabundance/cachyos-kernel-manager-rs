# PRIVILEGE MODEL

Reconstructed from the frozen oracle; the candidate's design follows.

## Oracle (v1.19.0)

```
user action → terminal-helper -s "pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh" <cmd>
            → terminal emulator runs:  pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh <tmpfile>
            → polkit action org.cachyos.KernelManager.pkexec.policy.run-root-terminal
              (allow_any=no, allow_inactive=no, allow_active=auth_admin)
            → /usr/lib/cachyos-kernel-manager/rootshell.sh   [annotated exec path]
            → exec /bin/bash "$@"
            → /bin/bash <tmpfile>                            [user-constructed command]
```

**Phase 5 differential witness** (transaction courts): the full chain is
captured from the real oracle GUI under strace on every transaction court,
e.g. `nvidia-companion/dkms-profile/oracle/oracle-trace.log`:

```
terminal-helper -s "pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh" \
                "pacman -S --needed ...; read -p 'Press enter to exit'"
  → mktemp
  → xterm -e pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh /tmp/tmp.XXXXXX
  → pkexec /usr/lib/cachyos-kernel-manager/rootshell.sh /tmp/tmp.XXXXXX
  → /bin/bash /tmp/tmp.XXXXXX
  → /usr/sbin/pacman -S --needed <install list>
```

The oracle's `utils::exec` probes run through glibc's `popen`, which on the
CachyOS toolchain (glibc ≥ 2.44) execs `sh -c -- <command>` (the `--` is a
glibc hardening addition; older glibc omits it — an environment-dependent
argv surface). The candidate's `exec_shell` reproduces the platform argv
exactly; the probe chain is compared witness-by-witness.

`rootshell.sh` is literally `exec /bin/bash "$@"`. This is an **arbitrary
root shell**: any caller who can trigger the polkit action can run any
command as root. The only protections are polkit's own authentication and the
fact that commands are constructed by the application.

Command construction sites (all through `runCmdTerminal(cmd, escalate=true)`):

| operation | argv payload |
|---|---|
| kernel install | `pacman -S --needed <list>` |
| kernel remove | `pacman -Rsn <list>` |

Package names in those lists come from libalpm (not user text) — except AUR
names (also package names from paru output). The `; read -p 'Press enter to
exit'` suffix is appended by the caller.

The build path escalates differently: `sudo pacman -U <globs>` typed inside
a **non-escalated** terminal (`run_cmd_async` without `-s`); `makepkg` itself
runs unprivileged. `sudo` here is the user's own sudo configuration.

## Candidate design

Prefer a narrow Rust privileged helper receiving a typed request. The
privileged surface becomes:

```rust
enum PrivilegedOperation {
    InstallRepoPackages { packages: Vec<PackageName>, needed: bool },
    RemovePackages { packages: Vec<PackageName>, recursive: bool },
}
```

The helper must:
- validate the request, reject unknown operations,
- validate package-name syntax (no slashes, no spaces, alphanumerics plus
  `-_.@+`; reject `..`, leading `-`, shell metacharacters),
- avoid shell interpretation (exec `pacman` via argv, not `/bin/sh -c`),
- use fixed executable paths,
- minimize inherited environment (clear `LD_*`, `BASH_ENV`, `ENV`, `PATH`
  to a fixed value),
- authenticate through polkit (same action ID), logging enough for forensic
  reconstruction without logging secrets.

## Compatibility shim

If keeping the polkit action identity
(`org.cachyos.KernelManager.pkexec.policy.run-root-terminal`) and the
annotated path is required for drop-in behavior, the candidate installs a
tiny shim at `/usr/lib/cachyos-kernel-manager/rootshell.sh` that:
- parses a **typed** request (e.g. first arg = opcode), and
- either executes the narrow helper or rejects.

The shim must be non-shell-expanding and covered by a privilege court. Any
narrowing of the oracle's arbitrary-root-shell design is a
`SECURITY_CORRECTION`, documented in `docs/KNOWN_DIVERGENCES.md`, never
silent. Reverse-compat: scripts or users invoking the old contract
(`rootshell.sh <script>`) are outside the current packaged contract (nothing
installs such callers), but the court `privilege/rootshell-argv` documents
the oracle behavior as executable witness.
