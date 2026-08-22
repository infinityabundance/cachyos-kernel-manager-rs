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

## Phase 8 rendering layer (Iced)

The UI crate (`cachyos-kernel-manager-ui`) is split into a presentation-free
semantic layer (courted) and the Iced rendering layer:

- **Semantic models** (always compiled): `strings.rs` (the full string
  inventory with file:line refs), `main_window.rs` (tree rows, enablement,
  sort keys), `configure_window.rs` (ctor defaults, variant switch, patch
  ops, save/load feed), `scx_window.rs` (the sched-ext window projection).
  Courted by `ui/*`.
- **Rendering layer** (feature `rendering`, i.e. the root `gui` feature):
  `app.rs` (the iced `Application`: `UiMessage` translates into the courted
  `AppEvent`s, the `Effect`s run as lazy `tokio::task::spawn_blocking`
  tasks), `i18n.rs` (embedded ts2json catalogs + the `initTranslations`
  resolution, courted by `ui/i18n-resolution`).

Window strategy: the oracle's three native windows render as a single-window
view stack — the Configure window replaces the main view while
`ConfigurationState::Editing`, the sched-ext window overlays while
`ScxState::Visible`, and the dialogs (progress/error/confirm) plus the
path editor (iced has no native file picker) render on top. The *semantics*
are courted; the choreography is a rendering choice.

Feature flags: the root `gui` feature enables `rendering`; `gui-alpm` adds
the real libalpm discovery + the scx D-Bus client (the alpm FFI cannot build
without system libalpm — CI verifies `gui`, the packaging layer builds
`gui-alpm`). Without `gui` the root binary keeps the foundation diagnostics
(`--diagnose`).

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
| 8 | Iced UI | **sealed** | The Phase 8 scope — the complete semantic UI over the pinned-down substrate — is SEALED: the orthogonal app-state refactor; the semantic models (main-window, configure-window, strings inventory, sched-ext window) courted by ui/dialog-strings + ui/main-window-semantics + ui/configure-window-semantics; the Iced rendering layer (tree with sortable headers, Configure Options/Patches tabs, sched-ext window, progress/error/confirm dialogs, the inline path dialogs; feature gui, gui-alpm adds real libalpm + scx dbus) verified by the 35-test rendering suite + the gui CI jobs; i18n courted by ui/i18n-resolution (initTranslations load order + the qrc alias set + QTranslator semantics; gap-009 zh_CN pinned); keyboard (space toggles the focused row) and accessibility (focus traversal + descriptive labels; an AT-SPI court for the running candidate GUI is blocked on an iced a11y bridge — gap-011). The close-during-transaction worker race (gap-010) is characterized and NOT reproduced (runtime-owned tasks). The oracle-side differential courts (Phases 2/5/6) continue to witness the oracle GUI against the semantic models; the full Phase 9 differential matrix is the next phase. |
| 9 | Full differential court matrix | **sealed** | The Phase 9 mandate — failure paths, historical regressions, repeated executions, drift/slew — is SEALED: single-instance/stale-lock PASS (gap-001: the QSharedMemory retry decision table + the real flock lock), drift-slew/pure-determinism PASS (all pure witnesses x3 fresh-process runs byte-identical), kernel-discovery/needle-order PASS (gap-002: the adversarial needle db row set + ORDER), build-env/makepkg-runtime PASS (gap-006: real makepkg runtime witness of the -s dep resolution, the -i install step, and the AUR-only failure), regression-suite/pure-regressions PASS (RES-2026-002/003/004/012 re-verified live, CI-wired). The close-during-transaction race (gap-010) is characterized (the oracle's latent use-after-free at km-window.cpp:327-338) and NOT reproduced (runtime-owned tasks); witnessing the oracle's actual crash behavior is race-hunting, explicitly Phase 12's mandate. |
| 10 | Packaging and migration | **sealed** | The Phase 10 scope — Arch package, drop-in files, package replacement, upgrade/revert courts — is SEALED: packaging/PKGBUILD (builds the Rust GUI with gui-alpm, installs the byte-identical drop-in surface), packaging/file-layout PASS (the candidate's installed file set == the frozen oracle package's 15-file surface; the 14 shared files byte-identical; the frozen package preserved in oracle/packages/, sha256 3dd688c6...), packaging/upgrade PASS (VM: oracle 1.19.0-1 -> candidate 0.1.0-1 -> oracle 1.19.0-1 via real pacman; the file surface + discovery rows preserved; the oracle's --version abort quirk documented in KNOWN_DIVERGENCES.md), the migration docs (docs/COMPATIBILITY.md §Migration: the cache + config schema unchanged; the candidate reads the oracle's state), and the forensic packaging matrix wired. The boot/system courts (Phase 11) follow. |
| 11 | Boot/system courts | pending | real kernel mutations, reboot, residual comparison |
| 12 | Hostile review | pending | security, fuzzing, race/mutation testing, dependency audit, unexplained residual hunt; production privilege replacement (D-001) |
| 13 | Release evidence | pending | parity ledger, known divergences, signed/hashable evidence pack (evidence/releases), reproducible release instructions |

