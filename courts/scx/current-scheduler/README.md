# scx/current-scheduler

Non-VM differential court for the oracle's `get_current_scheduler`
(`schedext-window-internal.cpp:57-72`): the candidate's `current_scheduler`
model (scx crate) vs the source-derived oracle reference, over a frozen
5-case corpus of sysfs file contents:

| corpus file | exercises |
|---|---|
| `disabled.json` | state `disabled` → the label is `disabled` (state text verbatim) |
| `enabled-bpfland.json` | state `enabled` + ops `scx_bpfland` → the ops text |
| `enabled-empty-ops.json` | state `enabled` + empty ops → `unknown` |
| `empty-state.json` | both reads empty → the empty state text |
| `weird-state.json` | a state that is not `enabled` → the state text (never the ops) |

The 1s timer (`update_current_sched`) refreshes this label even without
scx_loader — the surface is the kernel's own reporting, not the D-Bus.

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/current-scheduler` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/current-scheduler
```

Falsifier: any byte difference in the label JSON on any corpus case.
