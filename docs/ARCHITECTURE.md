# ARCHITECTURE

Native-Rust + Slint custodial reimplementation. Layered workspace; the domain
core never depends on any presentation technology.

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
  cachyos-kernel-manager-ui/       Slint application (Phase 8; imports core
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

- `core` (and everything below it) imports no presentation types (Slint,
  Qt) and no Qt.
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

## Phase 8 rendering layer (Slint)

The UI crate (`cachyos-kernel-manager-ui`) is split into a presentation-free
semantic layer (courted) and the Slint rendering layer:

- **Semantic models** (always compiled): `strings.rs` (the full string
  inventory with file:line refs), `main_window.rs` (tree rows, enablement,
  sort keys), `configure_window.rs` (ctor defaults, variant switch, patch
  ops, save/load feed), `scx_window.rs` (the sched-ext window projection).
  Courted by `ui/*`.
- **Rendering layer** (feature `rendering`, i.e. the root `gui` feature):
  `app.rs` (the Slint application: `UiMessage` translates into the courted
  `AppEvent`s, the `Effect`s run as lazy worker tasks), `i18n.rs` (embedded
  ts2json catalogs + the `initTranslations` resolution, courted by
  `ui/i18n-resolution`), the `.slint` window definitions in `ui/`.

Window strategy: the oracle's three native windows render as three separate
Slint windows (Main / Configure / SchedExt), with the dialogs
(progress/error/confirm) as OVERLAYS inside those windows (never separate
OS windows — they would show up as taskbar entries). The *semantics* are
courted; the choreography is a rendering choice. The windows carry stable
accessible ids so the installed binary can be driven via AT-SPI (slint's
accesskit bridge — the Phase 12 production integration closure).

Renderer: the default FemtoVG renderer is GPU-accelerated and requires
OpenGL; the GPU-less path is the winit-software renderer. The VM courts +
CI set `SLINT_BACKEND=winit-software` explicitly.

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
| 7 | SCX | **sealed** | typed org.scx.Loader client (zbus 5.19.0/zvariant 5.15.0 — the Slint port re-pinned the transport stack forward from the frozen authority's 5.5.0/5.4.0; the affected scx courts are re-run + re-sealed against the new lockfile, evidence release refreshed) + 8 scx courts PASS: button-visibility, current-scheduler, mode-flags, window-init, profile, apply, disable, loader-interface (non-VM source-derived from the recovered pre-extraction scx-manager f3eeaf6 + pinned scx_loader 1.0.9, AND the VM real-loader witness: the candidate's interface is a faithful subset of the shipped loader's, readback values match) |
| 8 | Slint UI | **sealed** | The Phase 8 scope — the complete semantic UI over the pinned-down substrate — is SEALED (re-sealed 2026-08-23 after the toolkit switch): the orthogonal app-state refactor; the semantic models (main-window, configure-window, strings inventory, sched-ext window) courted by ui/dialog-strings + ui/main-window-semantics + ui/configure-window-semantics; the SLINT rendering layer (feature gui, gui-alpm adds real libalpm + scx dbus) with the three native windows (main tree with sortable headers, Configure Options/Patches tabs, sched-ext window) + the inline path dialogs, verified by the rendering test suite + the layout-preview geometry test + the gui CI jobs; stable-identity kernel toggles (sorting never changes which kernel a click toggles), distinct window-lifecycle events (Configure cancel/close closes only that window), the production build contract (makepkg + sudo pacman -U run with the oracle's working-directory; .done-status removed at the oracle transition point; Configure cancel/close terminates the in-flight build process), the CachyOS green accent + icon + single taskbar entry (D-007 desktop StartupWMClass, normalized-witnessed by packaging/file-layout), i18n courted by ui/i18n-resolution (gap-009 zh_CN pinned; a CJK software-renderer rendering court is Phase 12 work — slint's software renderer limits text to western scripts); accessibility via slint's accesskit bridge — the running-candidate AT-SPI differential court (gap-011, formerly blocked on iced's missing a11y bridge) is now buildable and is the first Phase 12 slice. The close-during-transaction worker race (gap-010) is characterized and NOT reproduced (runtime-owned tasks). |
| 9 | Full differential court matrix | **sealed** | The Phase 9 mandate — failure paths, historical regressions, repeated executions, drift/slew — is SEALED: single-instance/stale-lock PASS (gap-001: the QSharedMemory retry decision table + the real flock lock), drift-slew/pure-determinism PASS (all pure witnesses x3 fresh-process runs byte-identical), kernel-discovery/needle-order PASS (gap-002: the adversarial needle db row set + ORDER), build-env/makepkg-runtime PASS (gap-006: real makepkg runtime witness of the -s dep resolution, the -i install step, and the AUR-only failure), regression-suite/pure-regressions PASS (RES-2026-002/003/004/012 re-verified live, CI-wired). The close-during-transaction race (gap-010) is characterized (the oracle's latent use-after-free at km-window.cpp:327-338) and NOT reproduced (runtime-owned tasks); witnessing the oracle's actual crash behavior is race-hunting, explicitly Phase 12's mandate. |
| 10 | Packaging and migration | **sealed** | The Phase 10 scope — Arch package, drop-in files, package replacement, upgrade/revert courts — is SEALED: packaging/PKGBUILD (builds the Rust GUI with gui-alpm, installs the byte-identical drop-in surface), packaging/file-layout PASS (the candidate's installed file set == the frozen oracle package's 15-file surface; the 14 shared files byte-identical; the frozen package preserved in oracle/packages/, sha256 3dd688c6...), packaging/upgrade PASS (VM: oracle 1.19.0-1 -> candidate 0.1.0-1 -> oracle 1.19.0-1 via real pacman; the file surface + discovery rows preserved; the oracle's --version abort quirk documented in KNOWN_DIVERGENCES.md), the migration docs (docs/COMPATIBILITY.md §Migration: the cache + config schema unchanged; the candidate reads the oracle's state), and the forensic packaging matrix wired. The boot/system courts (Phase 11) follow. |
| 11 | Boot/system courts | **sealed** | real kernel mutations, reboot, residual comparison. boot/system-boot-after-install PASS (VM: a REAL linux-cachyos-lts install via the courted command (literal == the model render) runs the REAL post-install hooks, the same overlay REBOOTS, boot-complete, the mutation persisted; the two boots' surfaces byte-identical; the kernel-install fixture caches the real lts packages offline); boot/system-boot-after-remove PASS (VM: sets up the two-kernel state, REMOVES the NON-running lts with the courted command, the hooks remove the lts initramfs, the same overlay REBOOTS, boot-complete, the lts kernel + its initramfs hard-asserted GONE; runner fixed to copy the installcmd witness for ALL boot courts and to run the reboot phase for the remove court); boot/system-boot-after-failure PASS (VM: REMOVES the RUNNING kernel; the reboot attempt is witnessed as the machine NOT becoming usable — no ssh within the bounded probe, boot-attempt.txt byte-identical on both sides — its own boot path destroyed); boot/system-boot-drift PASS (VM: the install mutation, then the SAME overlay reboots THREE times with a suffixed surface after each — byte-identical to each other and between sides; no drift). All four boot courts on fixture kernel-install, zero residuals. Phase 12 (hostile review) is next. |
| 12 | Hostile review | in progress | Phase 12 restructured (audit 2026-08-23): FIRST the production-integration closure (ui/gui-drive --vm — drive the PACKAGED binary, not witness CLIs), then hostile review: security, fuzzing, race/mutation testing, dependency audit, unexplained residual hunt; production privilege replacement (D-001). The audit P-list (P0 build-guard/cancel/SCX model seams, D-003 default-build grammar; P1 patch-refresh generation, per-child build env, fail-closed background tasks, transaction-boundary TOCTOU refresh, AUR executor cwd/~-expansion/stale-checkout, O_EXCL probe files, chwd dependency; P2 scx show-not-toggle, empty patch submission guards, D-009 licensing/MSRV) is IMPLEMENTED + tested. The production-integration slice is now CLOSED: ui/gui-drive PASS (2026-08-23, fixture gui-integration, at-spi2-core-2.52.0 pinned + the org.a11y.Status enablement stub — accesskit_unix 0.22.1 cannot serve full-tree updates past the first model rebuild, so the driver captures the header/row screen positions from the LIVE tree and delivers every click by XTEST, witnessing the sorted orders + toggled identities from the app's own KM_VERBOSE courted trace; the frozen Qt tree is the oracle witness). The court itself caught and fixed TWO candidate divergences: the discovery order (the candidate iterated alpm_db_get_pkgcache while the oracle's Kernel::get_kernels iterates alpm_db_search results — the FFI now binds alpm_db_search and the sync db packages are assembled in search order) and the sort semantics (the oracle's QTreeWidget _q_headerClicked RESETS a new column to ascending and re-sorts the DISPLAYED items — the previous order is the stable tie-break — while the candidate kept the current order and re-sorted the discovery catalog; both fixed + unit-tested). Full VM matrix re-run: 38/38 VM courts PASS (fail-closed aggregator vm/run-vm-matrix.sh). Remaining Phase 12: hostile review (fuzzing, race hunts, D-001 privilege replacement, CJK software-renderer court). |
| 13 | Release evidence | pending | parity ledger, known divergences, signed/hashable evidence pack (evidence/releases), reproducible release instructions |

