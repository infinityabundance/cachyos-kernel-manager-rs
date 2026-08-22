# cachyos-kernel-manager-rs

Native Rust + Iced custodial reimplementation of
[CachyOS Kernel Manager](https://github.com/CachyOS/kernel-manager),
built under the Forensic Residual Framework (FRF) methodology:
behavior is proven by differential execution against the frozen upstream
oracle in disposable VMs, and every discrepancy is treated as evidence.

> The quality target: a skeptical CachyOS maintainer should be able to
> inspect this repository and see exactly why trusting the Rust
> implementation would be reasonable.

> The shipped cachyos-kernel-manager binary is the Phase 8 Iced GUI (feature `gui`; `gui-alpm` adds the real libalpm discovery + the scx D-Bus client). The foundation diagnostics remain behind `--diagnose`. Nothing beyond the sealed phases is claimed complete.

## Status

| phase | scope | status |
|---|---|---|
| 0 | Freeze authority | **sealed** | oracle v1.19.0 (6b4a373e) frozen; source archive + package hashes hash-verified (oracle/UPSTREAM.lock) |
| 1 | Build atlas | **sealed** | atlas/inventory.json (37 surfaces), court ledger, residual ledger, coverage gaps, docs/* archaeology |
| 2 | Oracle instrumentation | **sealed** | docker+qemu VM farm, fixture baker, AT-SPI observation, FRF comparator + evidence; all 10 baseline discovery courts PASS |
| 3 | Pure domain core | **sealed** | discovery, version state, category, options/transitions, selection, app state machine; 100+ tests; refinement deferred to Phase 9 |
| 4 | ALPM layer | **sealed** | isolated FFI with a build-time ABI court (abi/probe.c: list layout, enum sizes, every extern signature) + the alpm-ffi/abi-surface evidence court; mINI pacman.conf port; all Phase 4 courts PASS |
| 5 | Execution/privilege | **sealed** | execution/privilege SEMANTICS and oracle characterization sealed: plan/exec layers, terminal-helper matrix, polkit identity, differential GUI transaction courts (strace exec-chain witnesses); 10 Phase 5 courts PASS. The PRODUCTION privilege replacement (narrow typed helper) is planned, not implemented (D-001). |
| 6 | Build subsystem | **sealed** | PKGBUILD mutation models + courts (patch-injection/source-array, custom-name/pkgbase-injection), artifact-glob/package-functions, build-env/env-rendering + lifecycle + failure-lifecycle + cancellation, option-transitions/variant-switch, git-cache/lifecycle, config-roundtrip/canonicalization, aur/enablement-matrix (discovery gating + commit ordering; the meson-vs-CMake flag difference documented); all Phase 6 courts PASS, 112 workspace tests |
| 7 | SCX | **sealed** | typed org.scx.Loader client (zbus 5.5.0/zvariant 5.4.0 = the frozen authority's exact versions) + 8 scx courts PASS: button-visibility, current-scheduler, mode-flags, window-init, profile, apply, disable, loader-interface (non-VM source-derived from the recovered pre-extraction scx-manager f3eeaf6 + pinned scx_loader 1.0.9, AND the VM real-loader witness: the candidate's interface is a faithful subset of the shipped loader's, readback values match) |
| 8 | Iced UI | **sealed** | The Phase 8 scope — the complete semantic UI over the pinned-down substrate — is SEALED: the orthogonal app-state refactor; the semantic models (main-window, configure-window, strings inventory, sched-ext window) courted by ui/dialog-strings + ui/main-window-semantics + ui/configure-window-semantics; the Iced rendering layer (tree with sortable headers, Configure Options/Patches tabs, sched-ext window, progress/error/confirm dialogs, the inline path dialogs; feature gui, gui-alpm adds real libalpm + scx dbus) verified by the 35-test rendering suite + the gui CI jobs; i18n courted by ui/i18n-resolution (initTranslations load order + the qrc alias set + QTranslator semantics; gap-009 zh_CN pinned); keyboard (space toggles the focused row) and accessibility (focus traversal + descriptive labels; an AT-SPI court for the running candidate GUI is blocked on an iced a11y bridge — gap-011). The close-during-transaction worker race (gap-010) is characterized and NOT reproduced (runtime-owned tasks). The oracle-side differential courts (Phases 2/5/6) continue to witness the oracle GUI against the semantic models; the full Phase 9 differential matrix is the next phase. |
| 9 | Full differential court matrix | in progress | failure paths, historical regressions, repeated executions, drift/slew. Done: single-instance/stale-lock PASS (gap-001 covered), drift-slew/pure-determinism PASS (all pure witnesses x3 fresh-process runs byte-identical), kernel-discovery/needle-order PASS (gap-002 covered), build-env/makepkg-runtime PASS (gap-006 covered: real makepkg runtime witness of -s dep resolution vs the -i install vs the AUR-only failure), regression-suite/pure-regressions PASS (RES-2026-002/003/004/012 re-verified live, wired into CI). Remaining: the close-during-transaction race witness (gap-010) — the oracle's latent use-after-free is characterized and NOT reproduced (runtime-owned tasks); a VM court for the oracle's actual crash behavior is the last Phase 9 item. |
| 10 | Packaging and migration | pending | Arch package, drop-in files, package replacement, upgrade/revert courts |
| 11 | Boot/system courts | pending | real kernel mutations, reboot, residual comparison |
| 12 | Hostile review | pending | security, fuzzing, race/mutation testing, dependency audit, unexplained residual hunt; production privilege replacement (D-001) |
| 13 | Release evidence | pending | parity ledger, known divergences, signed/hashable evidence pack (evidence/releases), reproducible release instructions |


## Quick start

```sh
cargo build                     # builds the workspace (the GUI needs `--features gui`)
cargo build --features gui-alpm # the full Phase 8 GUI (real libalpm + scx dbus)
cargo test --workspace         # 167 unit/property tests over the reconstructed semantics
cargo test -p cachyos-kernel-manager-ui --features rendering --release  # + 35 GUI/i18n tests
cargo xtask oracle verify   # verifies the frozen source archive hash
cargo xtask oracle info     # prints the frozen authority record
cargo xtask court list      # lists court case directories
cargo xtask court run --all # runs the pure courts whose fixtures are present
cargo xtask court run <case> --vm  # differential VM court (real oracle GUI)
cargo xtask upstream diff <ref>  # diff locked oracle vs a candidate ref
```

## What the oracle is (v1.19.0, frozen)

- Qt6/C++23 app; `libalpm` authoritative for package state; `pacman` for
  transactions; polkit `org.cachyos.KernelManager.pkexec.policy.run-root-terminal`
  via an arbitrary-root-shell helper (`rootshell.sh` = `exec /bin/bash "$@"`);
  terminal-helper chain for interactive pacman/makepkg; TOML build config
  schema defined by an in-tree Rust crate (`config-option-lib`, cxxbridge);
  sched-ext UI from `scxctl-ui`; AUR kernels behind `ENABLE_AUR_KERNELS`.

Full archaeology: `docs/UPSTREAM_ARCHAEOLOGY.md`.
Machine-readable surface inventory: `atlas/inventory.json`.

## Repository layout

```
oracle/     frozen upstream clone + deterministic source archive + UPSTREAM.lock
atlas/      inventory.json, court ledger, residual ledger, coverage gaps
courts/     reproducible FRF parity-court case directories
crates/     layered workspace (core → plan/exec/build/config → ui; see docs/ARCHITECTURE.md)
docs/       the custodial documentation set
vm/         VM image definitions (Phase 2)
fixtures/   static fixture corpora (Phase 2+)
packaging/  Arch package (Phase 10)
xtask/      Rust-native orchestration
```

## License

GPL-2.0-or-later (matching the upstream headers; note upstream ships GPLv3
text in its LICENSE while every source header says "version 2 or later" —
recorded in the archaeology docs).
