# drift-slew/pure-determinism

Non-VM self-consistency court for the Phase 9 **repeated-executions /
drift-slew** guarantee: every pure corpus-driven witness CLI runs **three
times** over its frozen corpus (fresh processes), and all three runs must be
byte-identical.

- **oracle/** — run 1 of each witness (the reference execution);
- **candidate/** — run 2 of the same witness (must equal run 1);
- **run 3** — diffed against run 1 *inside the runner* (`tools/run-drift
  -corpus.sh`): any byte difference is a hard runner failure.

Witnesses covered:

- `cachyos-kernel-manager-config` (config-roundtrip/canonicalization)
- `cachyos-kernel-manager-variant-switch` (option-transitions/variant-switch)
- `cachyos-kernel-manager-buildflow` (build-env/lifecycle)
- `cachyos-kernel-manager-env` (build-env/env-rendering)
- `cachyos-kernel-manager-mainwindow` / `-confwindow` / `-i18n`
  (ui/main-window-semantics, ui/configure-window-semantics, ui/i18n-resolution)
- `cachyos-kernel-manager-single-instance` (single-instance/stale-lock)
- `cachyos-kernel-manager-strings` (ui/dialog-strings — the fixed table)

Each witness's parity with the frozen source is courted by its own court;
this court pins the **across-execution determinism** (the VM courts'
determinism is witnessed by the forensic workflow's re-runs on identical
overlays — the Phase 5 minimal ×3 / upgrade-available ×2 runs).

Status: defined. Run:

```
tools/run-drift-corpus.sh
cargo xtask court run drift-slew/pure-determinism
```

Falsifier: any byte difference between runs 1 and 2 (a residual) or
between runs 1 and 3 (a runner failure) of any witness over any corpus
file.
