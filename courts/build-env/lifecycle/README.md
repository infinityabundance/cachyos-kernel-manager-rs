# build-env/lifecycle

Non-VM differential court for the build flow (§25, gap-006): the
candidate's `BuildFlowPlan` (exec crate) vs the oracle's `on_execute` +
`finished_proc` + `aur_kernel.cpp` decisions (`conf-window.cpp:696-735`,
`378-405`), over a frozen 6-case corpus of (variant, cwd, globs):

| corpus file | exercises |
|---|---|
| `cachyos-tmp.json` | the VM launch cwd (/tmp), split-package globs |
| `server-home.json` | server variant + user home cwd |
| `rt-vm.json` | rt -> linux-cachyos-rt-bore path, 3 globs |
| `hardened-root.json` | root cwd |
| `empty-globs.json` | `sudo pacman -U ` with no globs (edge) |
| `lts-odd-cwd.json` | a cwd with spaces |

Covers: cpusched_path mapping, the mutable-cwd working_path quirk (D-004),
the repo build command (`-scf`, `&& touch .done-status` — success by marker,
not exit code), the run_cmd_async terminal-helper argv (pause suffix, no
`-s`), the AUR build command (`-sicf`, gap-006), and the artifact-install
command (`sudo pacman -U <globs>`).

Witness: `tools/run-buildflow-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run build-env/lifecycle` byte-compares them.

Status: defined. Run:

```
tools/run-buildflow-corpus.sh
cargo xtask court run build-env/lifecycle
```

Falsifier: any byte difference in any field of the plan JSON on any corpus
case.
