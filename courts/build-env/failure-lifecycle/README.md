# build-env/failure-lifecycle

Non-VM differential court for the oracle's `finished_proc`
(`conf-window.cpp:378-405`): the candidate's `finished_proc` model (exec
crate) vs the source-derived oracle reference, over a frozen 7-case corpus
of async-process completion sequences:

| corpus file | exercises |
|---|---|
| `build-success-yes.json` | the success path with the install question answered Yes |
| `build-success-no.json` | the success path with No (no install) |
| `build-failure-exit1.json` | `.done-status` absent → the failure stderr line |
| `file-present-exit-nonzero.json` | success keys on the FILE, not the exit code |
| `success-then-install-reentry.json` | the re-entrant install's OWN completion → `process failed with exit code: 0` even on pacman success (quirk) |
| `multiple-globs.json` | `sudo pacman -U <all globs joined by ' '>` |
| `empty-events.json` | no completions → empty trace |

Covers: the `.done-status`-presence decision (the success contract, not the
exit code), the stdout lines (`success`, `pressed yes`,
`pacman_cmd := ...`), the stderr line (`process failed with exit code: <n>`
with the oracle's trailing `\n`), the re-entrant install command, the
`.done-status` removal, and the `m_running` transitions.

Witness: `tools/run-finish-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run build-env/failure-lifecycle` byte-compares them.

Status: defined. Run:

```
tools/run-finish-corpus.sh
cargo xtask court run build-env/failure-lifecycle
```

Falsifier: any byte difference in any outcome field on any corpus case.
