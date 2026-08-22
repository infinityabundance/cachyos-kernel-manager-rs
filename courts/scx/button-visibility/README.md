# scx/button-visibility

Non-VM differential court for the main window's sched-ext button visibility
(`km-window.cpp:185-188`): the candidate's `main_window_schedext_visible`
(scx crate) vs the source-derived oracle reference, over a frozen 2-case
corpus:

| corpus file | exercises |
|---|---|
| `present.json` | `/sys/kernel/sched_ext/state` exists → the button is visible |
| `absent.json` | the file is absent → the button is hidden (`setHidden(true)`) |

The file-present direction is additionally VM-witnessed by the existing
`kernel-discovery/minimal` evidence: the a11y tree of the real GUI on the
VM's sched-ext kernel lists the `sched-ext scheduler config` push button
with the `showing` + `visible` states. The file-absent direction cannot be
VM-witnessed (CachyOS kernels ship sched-ext), so this court pins the
DECISION for both directions.

Witness: `tools/run-scx-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run scx/button-visibility` byte-compares them.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/button-visibility
```

Falsifier: any byte difference in the visibility JSON on any corpus case.
