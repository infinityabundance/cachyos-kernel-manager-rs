# scx/apply

Non-VM differential court for the oracle's `apply_scheduler_change`
(`config-option-lib/src/scx_loader_config.rs` at the pre-extraction commit
`f3eeaf6`): the candidate's `apply_trace` model (scx crate) vs the
source-derived oracle reference, over a frozen 10-case corpus:

| corpus file | exercises |
|---|---|
| `mode-path-default-flags.json` | flags == the mode defaults → `switch_scheduler` (the mode path) |
| `args-path-custom.json` | custom flags → `switch_scheduler_with_args` |
| `args-path-empty.json` | no flags + Gaming → the args path with an empty list (the oracle still goes there) |
| `auto-mode.json` | Auto defaults to `[]` → the mode path with empty flags |
| `scx-service-enabled.json` | `scx` enabled → `systemctl disable --now -f scx` + `Disabling scx service` |
| `scx-service-active.json` | `scx` active → `systemctl stop -f scx` + the oracle's `Stoping scx service` typo |
| `loader-not-enabled.json` | → `Enabling scx_loader service` + `systemctl enable -f scx_loader` |
| `db-fail-mode.json` | D-Bus failure on the mode path → the failure stdout line; persist still runs |
| `db-fail-args.json` | D-Bus failure on the args path |
| `full-combo.json` | everything at once: scx enabled+active, custom flags, loader not enabled, D-Bus failure |

Covers: the service-conflict branch (enabled wins over active), the
args-vs-mode decision (commit b70b01b: args only when they differ from the
mode defaults), the oracle's stdout lines byte-for-byte (including the
`Stoping` typo and the Rust Debug renderings), the loader-enable branch,
and the final `pkexec /usr/bin/cp /tmp/scx_loader.toml <config_path>`
persist (which runs even on D-Bus failure).

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/apply` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/apply
```

Falsifier: any byte difference in any step of the apply trace on any
corpus case.
