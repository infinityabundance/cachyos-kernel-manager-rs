# scx/disable

Non-VM differential court for the oracle's `disable_scheduler`
(`config-option-lib/src/scx_loader_config.rs` at the pre-extraction commit
`f3eeaf6`): the candidate's `disable_trace` + `disable_config_mutation`
models (scx crate) vs the source-derived oracle reference, over a frozen
3-case corpus:

| corpus file | exercises |
|---|---|
| `with-default-sched.json` | a config with default_sched set → before `scx_bpfland`, after None |
| `no-default-sched.json` | default config → before None, after None |
| `empty-config.json` | a config with only a scheds section |

Covers: the `stop_scheduler()` D-Bus call, the `pkexec /usr/bin/cp
/tmp/scx_loader.toml <config_path>` copy, and the config mutation
(`default_sched = None`; `default_mode` untouched).

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/disable` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/disable
```

Falsifier: any byte difference in the disable trace or the config mutation
on any corpus case.
