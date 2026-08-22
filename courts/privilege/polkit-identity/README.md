# privilege/polkit-identity

Drop-in polkit identity court: the candidate's packaged
`org.cachyos.KernelManager.pkexec.policy` must be byte-identical to the
frozen upstream policy.

| surface | value |
|---|---|
| action id | `org.cachyos.KernelManager.pkexec.policy.run-root-terminal` |
| allow_any / allow_inactive / allow_active | no / no / auth_admin |
| exec.path annotation | `/usr/lib/cachyos-kernel-manager/rootshell.sh` |
| vendor | cachyos-kernel-manager |

The privilege model (typed-helper design, SECURITY_CORRECTION notes) is
documented in `docs/PRIVILEGE_MODEL.md`.

Run: `cargo xtask court run privilege/polkit-identity`
