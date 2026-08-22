# scx/mode-flags

Non-VM differential court for the oracle's `get_scx_flags_for_mode`
(`scx_loader 1.0.9 src/config.rs:101-116,169-189`): the candidate's
`flags_for_mode` model (scx crate) vs the source-derived oracle reference,
over a frozen 12-case corpus:

| corpus file | exercises |
|---|---|
| `bpfland-gaming.json` | `-m performance` |
| `bpfland-lowlatency.json` | `-s 5000 -S 500 -l 5000 -m performance` |
| `bpfland-powersave.json` | `-m powersave` |
| `bpfland-server.json` | `-p` |
| `bpfland-auto.json` | `[]` |
| `lavd-gaming.json` | `--performance` |
| `lavd-powersave.json` | `--powersave` |
| `lavd-server.json` | `[]` |
| `rusty-gaming.json` | `[]` (rusty supports no modes) |
| `flash-server.json` | `[]` (flash supports no modes) |
| `config-override.json` | the config entry's `gaming_mode` overrides the default |
| `config-absent-field.json` | the config entry lacks the mode's field → hardcoded fallback |

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/mode-flags` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/mode-flags
```

Falsifier: any byte difference in the flags JSON on any corpus case.
