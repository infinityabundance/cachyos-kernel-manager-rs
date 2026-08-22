# scx/window-init

Non-VM differential court for the oracle's SchedExtWindow constructor
(`schedext-window-internal.cpp:120-190`): the candidate's `window_init`
model (scx crate) vs the source-derived oracle reference, over a frozen
5-case corpus:

| corpus file | exercises |
|---|---|
| `loader-ok-default.json` | loader up, default config → the full populate trace (default: no scheduler selected, Auto, `disabled` label) |
| `loader-ok-configured.json` | loader up, config with default_sched=scx_bpfland + Gaming → initial values + profile visible + Gaming flags |
| `no-loader.json` | `get_supported_scheds` fails → critical dialog + hidden widgets + stop |
| `bad-config.json` | config init fails → critical dialog + stop (empty window) |
| `loader-ok-empty-scheds.json` | loader up with an empty supported list |

Covers: the two stop paths (config-init failure, loader failure), the
scheduler combo population from the D-Bus list, the initial scheduler/mode
from the config defaults, the fixed 5-item profile combo, the running
scheduler label, the profile-visibility decision, and the initial flags
render.

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/window-init` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/window-init
```

Falsifier: any byte difference in any step of the init trace on any corpus
case.
