# ARCHITECTURE

Native-Rust + Iced custodial reimplementation. Layered workspace; the domain
core never depends on Iced.

## Workspace layout

```
crates/
  cachyos-kernel-manager-core/     pure domain: kernel model, version state,
                                   category classifier, discovery rules,
                                   options model, app state machine
  cachyos-kernel-manager-alpm/     typed adapter over system libalpm (small
                                   unsafe FFI boundary, forbid(unsafe) except
                                   the binding module)
  cachyos-kernel-manager-plan/     TransactionPlan, PackageAction, Reason,
                                   plan construction from selection state
  cachyos-kernel-manager-exec/     typed CommandPlan + argv rendering,
                                   terminal/rootshell chain modeling,
                                   process execution behind a trait
  cachyos-kernel-manager-build/    PKGBUILD mutation, env rendering, artifact
                                   globs, .done-status lifecycle (pure)
  cachyos-kernel-manager-config/   TOML config schema (parity with
                                   config-option-lib), load/save, corpus
  cachyos-kernel-manager-scx/      typed org.scx.Loader client (Phase 7)
  cachyos-kernel-manager-platform/ paths, env, os, single-instance lock
  cachyos-kernel-manager-i18n/     locale resolution, catalog model
  cachyos-kernel-manager-ui/       Iced application (Phase 8; imports core
                                   types only, never the reverse)
  cachyos-kernel-manager-casefile/ court case model, comparators, residual
                                   ledger, evidence hashing
  cachyos-kernel-manager-oracle/   UPSTREAM.lock model, revision diffing
  cachyos-kernel-manager-frf/      FRF receipt/evidence chain types
xtask/                             Rust-native orchestration
src/main.rs                        binary entry point
oracle/                            frozen upstream + lock + archives
courts/                            reproducible court case directories
atlas/                             machine-readable surface inventory
fixtures/                          static fixture corpora (configs, PKGBUILDs)
vm/                                VM image definitions (Phase 2)
packaging/                         PKGBUILD + install layout (Phase 10)
docs/                              this documentation set
```

## Layering rules

- `core` (and everything below it) imports no Iced types and no Qt.
- Dependency direction: `ui → casefile? no — ui → core/plan/build/config/
  exec → platform → (alpm|scx)`.
- All `unsafe` lives in the alpm binding module; every other crate is
  `#![forbid(unsafe_code)]`.
- External authorities stay external: libalpm for package state, pacman for
  transactions, bash/makepkg for PKGBUILD semantics, D-Bus+scx_loader for
  sched-ext. We model commands before rendering them; shell construction is
  confined to the execution adapter.

## State machine

The application is a pure transition function `State + Event → (State,
Effects)`. States (from the directive, mapped to oracle evidence):

| state | oracle evidence |
|---|---|
| startup | main(): lock, QApplication init, org/app names |
| kernel discovery | blocking in MainWindow ctor (oracle) — candidate makes it async |
| ready | tree populated; ok disabled |
| selection changed | build_change_list semantics |
| transaction planning | install_packages/remove_packages + Kernel::install/remove expansion |
| authentication | pkexec polkit prompt (escalated terminal path) |
| transaction running | worker thread; ok disabled |
| transaction complete | is_kernels_change_state → re-discovery |
| transaction failed | alpm error dialogs; .done-status absent |
| refreshing package state | post-transaction re-init |
| configuration preparation | QtConcurrent git refresh + progress dialog |
| configuration editing | Options/Patches tabs |
| build running | QProcess terminal-helper async |
| build completed | .done-status present |
| build failed | .done-status absent → stderr message |
| artifact installation | sudo pacman -U prompt |
| SCX configuration | SchedExtWindow |
| shutdown | closeEvent |

Effects are explicit values (spawn process, run command plan, show dialog,
persist config) so the machine is replayable and testable.

## Phase status

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
| 8 | Iced UI | pending | complete semantic UI over the pinned-down substrate: keyboard, dialogs, progress, i18n, accessibility |
| 9 | Full differential court matrix | pending | failure paths, historical regressions, repeated executions, drift/slew |
| 10 | Packaging and migration | pending | Arch package, drop-in files, package replacement, upgrade/revert courts |
| 11 | Boot/system courts | pending | real kernel mutations, reboot, residual comparison |
| 12 | Hostile review | pending | security, fuzzing, race/mutation testing, dependency audit, unexplained residual hunt; production privilege replacement (D-001) |
| 13 | Release evidence | pending | parity ledger, known divergences, signed/hashable evidence pack (evidence/releases), reproducible release instructions |

