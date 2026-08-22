# scx/profile

Non-VM differential court for the oracle's profile-selection semantics
(`on_sched_changed` + `on_sched_profile_changed`,
`schedext-window-internal.cpp:250-264,199-214`): the candidate's
`profile_ui_visible` + `flags_text` models (scx crate) vs the
source-derived oracle reference, over a frozen 6-case corpus:

| corpus file | exercises |
|---|---|
| `bpfland-gaming.json` | bpfland → profile UI visible; Gaming flags `-m performance` |
| `bpfland-lowlatency.json` | the lowlatency flag set |
| `lavd-powersave.json` | lavd → visible; `--powersave` |
| `rusty-gaming.json` | rusty → profile UI HIDDEN; no flags |
| `flash-server.json` | flash → hidden; no flags |
| `config-override.json` | the config entry's gaming_mode overrides the flags text |

The "BMQ restriction" analog: only scx_bpfland and scx_lavd support
different preset profiles.

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/profile` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/profile
```

Falsifier: any byte difference in the profile JSON on any corpus case.
