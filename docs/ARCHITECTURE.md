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
| 0 | freeze authority | DONE (6b4a373e / v1.19.0) |
| 1 | atlas/archaeology | DONE (atlas/inventory.json, docs/*) |
| 2 | oracle instrumentation (VMs) | PENDING (vm/ + xtask oracle) |
| 3 | pure domain core | IN PROGRESS (this milestone: models + tests) |
| 4 | ALPM layer | SKELETON (binding module stubbed behind feature) |
| 5 | execution/privilege | MODELED (exec crate); helper implementation pending |
| 6 | build subsystem | MODELS DONE (build crate); process wiring pending |
| 7 | SCX | PENDING |
| 8 | Iced UI | PENDING (ui crate defines messages/state) |
| 9 | differential courts | PENDING (courts/ format defined) |
| 10 | packaging | PENDING |
| 11 | boot courts | PENDING |
| 12 | hostile review | PENDING |
| 13 | release evidence | PENDING |

Nothing beyond its phase is claimed complete.
