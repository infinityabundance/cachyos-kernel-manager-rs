# privilege/helper-scripts

Byte-identity court for the two installed Bash helper scripts:

| path | purpose |
|---|---|
| `/usr/lib/cachyos-kernel-manager/terminal-helper` | terminal-emulator launcher (exit-code surface courted by `terminal-helper/emulator-matrix`) |
| `/usr/lib/cachyos-kernel-manager/rootshell.sh` | `exec /bin/bash "$@"` — the polkit-annotated escalation chain |

Both are GPL-2.0-or-later upstream files (provenance: `oracle/upstream/src/`,
revision `6b4a373e`); the candidate keeps them byte-identical rather than
reimplementing Bash semantics in Rust.

Run: `cargo xtask court run privilege/helper-scripts`
