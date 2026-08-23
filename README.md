# cachyos-kernel-manager-rs

Native Rust + Slint custodial reimplementation of
[CachyOS Kernel Manager](https://github.com/CachyOS/kernel-manager),
built under the Forensic Residual Framework (FRF) methodology:
behavior is proven by differential execution against the frozen upstream
oracle in disposable VMs, and every discrepancy is treated as evidence.

> The quality target: a skeptical CachyOS maintainer should be able to
> inspect this repository and see exactly why trusting the Rust
> implementation would be reasonable.

> The shipped cachyos-kernel-manager binary is the Phase 8 Slint GUI (feature `gui`; `gui-alpm` adds the real libalpm discovery + the scx D-Bus client), rendered with winit + the winit-software renderer (`SLINT_BACKEND=winit-software` in the VM courts) so no GL/Vulkan is required. The foundation diagnostics remain behind `--diagnose`. Nothing beyond the sealed phases is claimed complete.

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
| 7 | SCX | **sealed** | typed org.scx.Loader client (zbus 5.19.0/zvariant 5.15.0 — the Slint port re-pinned the transport stack forward from the frozen authority's 5.5.0/5.4.0; the affected scx courts are re-run + re-sealed against the new lockfile, evidence release refreshed) + 8 scx courts PASS: button-visibility, current-scheduler, mode-flags, window-init, profile, apply, disable, loader-interface (non-VM source-derived from the recovered pre-extraction scx-manager f3eeaf6 + pinned scx_loader 1.0.9, AND the VM real-loader witness: the candidate's interface is a faithful subset of the shipped loader's, readback values match) |
| 8 | Slint UI | **sealed** | The Phase 8 scope — the complete semantic UI over the pinned-down substrate — is SEALED (re-sealed 2026-08-23 after the toolkit switch): the orthogonal app-state refactor; the semantic models (main-window, configure-window, strings inventory, sched-ext window) courted by ui/dialog-strings + ui/main-window-semantics + ui/configure-window-semantics; the SLINT rendering layer (feature gui, gui-alpm adds real libalpm + scx dbus) with the three native windows (main tree with sortable headers, Configure Options/Patches tabs, sched-ext window) + the inline path dialogs, verified by the rendering test suite + the layout-preview geometry test + the gui CI jobs; stable-identity kernel toggles (sorting never changes which kernel a click toggles), distinct window-lifecycle events (Configure cancel/close closes only that window), the production build contract (makepkg + sudo pacman -U run with the oracle's working-directory; .done-status removed at the oracle transition point; Configure cancel/close terminates the in-flight build process), the CachyOS green accent + icon + single taskbar entry (D-007 desktop StartupWMClass, normalized-witnessed by packaging/file-layout), i18n courted by ui/i18n-resolution (gap-009 zh_CN pinned; a CJK software-renderer rendering court is Phase 12 work — slint's software renderer limits text to western scripts); accessibility via slint's accesskit bridge — the running-candidate AT-SPI differential court (gap-011, formerly blocked on iced's missing a11y bridge) is now buildable and is the first Phase 12 slice. The close-during-transaction worker race (gap-010) is characterized and NOT reproduced (runtime-owned tasks). |
| 9 | Full differential court matrix | **sealed** | The Phase 9 mandate — failure paths, historical regressions, repeated executions, drift/slew — is SEALED: single-instance/stale-lock PASS (gap-001: the QSharedMemory retry decision table + the real flock lock), drift-slew/pure-determinism PASS (all pure witnesses x3 fresh-process runs byte-identical), kernel-discovery/needle-order PASS (gap-002: the adversarial needle db row set + ORDER), build-env/makepkg-runtime PASS (gap-006: real makepkg runtime witness of the -s dep resolution, the -i install step, and the AUR-only failure), regression-suite/pure-regressions PASS (RES-2026-002/003/004/012 re-verified live, CI-wired). The close-during-transaction race (gap-010) is characterized (the oracle's latent use-after-free at km-window.cpp:327-338) and NOT reproduced (runtime-owned tasks); witnessing the oracle's actual crash behavior is race-hunting, explicitly Phase 12's mandate. |
| 10 | Packaging and migration | **sealed** | The Phase 10 scope — Arch package, drop-in files, package replacement, upgrade/revert courts — is SEALED: packaging/PKGBUILD (builds the Rust GUI with gui-alpm, installs the byte-identical drop-in surface), packaging/file-layout PASS (the candidate's installed file set == the frozen oracle package's 15-file surface; the 14 shared files byte-identical; the frozen package preserved in oracle/packages/, sha256 3dd688c6...), packaging/upgrade PASS (VM: oracle 1.19.0-1 -> candidate 0.1.0-1 -> oracle 1.19.0-1 via real pacman; the file surface + discovery rows preserved; the oracle's --version abort quirk documented in KNOWN_DIVERGENCES.md), the migration docs (docs/COMPATIBILITY.md §Migration: the cache + config schema unchanged; the candidate reads the oracle's state), and the forensic packaging matrix wired. The boot/system courts (Phase 11) follow. |
| 11 | Boot/system courts | **sealed** | real kernel mutations, reboot, residual comparison. boot/system-boot-after-install PASS (VM: a REAL linux-cachyos-lts install via the courted command (literal == the model render) runs the REAL post-install hooks, the same overlay REBOOTS, boot-complete, the mutation persisted; the two boots' surfaces byte-identical; the kernel-install fixture caches the real lts packages offline); boot/system-boot-after-remove PASS (VM: sets up the two-kernel state, REMOVES the NON-running lts with the courted command, the hooks remove the lts initramfs, the same overlay REBOOTS, boot-complete, the lts kernel + its initramfs hard-asserted GONE; runner fixed to copy the installcmd witness for ALL boot courts and to run the reboot phase for the remove court); boot/system-boot-after-failure PASS (VM: REMOVES the RUNNING kernel; the reboot attempt is witnessed as the machine NOT becoming usable — no ssh within the bounded probe, boot-attempt.txt byte-identical on both sides — its own boot path destroyed); boot/system-boot-drift PASS (VM: the install mutation, then the SAME overlay reboots THREE times with a suffixed surface after each — byte-identical to each other and between sides; no drift). All four boot courts on fixture kernel-install, zero residuals. Phase 12 (hostile review) is next. |
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
