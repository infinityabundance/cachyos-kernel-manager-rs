# cachyos-kernel-manager-rs

Native Rust + Iced custodial reimplementation of
[CachyOS Kernel Manager](https://github.com/CachyOS/kernel-manager),
built under the Forensic Residual Framework (FRF) methodology:
behavior is proven by differential execution against the frozen upstream
oracle in disposable VMs, and every discrepancy is treated as evidence.

> The quality target: a skeptical CachyOS maintainer should be able to
> inspect this repository and see exactly why trusting the Rust
> implementation would be reasonable.

## Status — Phase 0/1 done, Phase 3 foundation built

| phase | scope | status |
|---|---|---|
| 0 | freeze authority | **done** — `oracle/UPSTREAM.lock` (v1.19.0, `6b4a373e`, deterministic archive `1e464db6…`, package hash `3dd688c6…`, hash-verified) |
| 1 | archaeology + atlas | **done** — `atlas/inventory.json` (37 surfaces), `docs/*` (full source archaeology), git-history lore |
| 2 | VM oracle instrumentation | **in progress** — docker+qemu image builder, fixture baker (10 baseline fixtures), SSH/9p harness, AT-SPI observation, candidate inspect tool (libalpm FFI), FRF comparator + evidence; base image building |
| 3 | pure domain core | **built** — discovery, version state, category, options/transitions, selection, app state machine; 80 tests |
| 4 | ALPM layer | **built (minimal)** — isolated FFI (`alpm_initialize/register/db_get_pkg/pkgcache/vercmp/installed_db`), mINI pacman.conf port, null backend; transaction API deliberately excluded |
| 5–13 | exec/build/…/release | partially modeled (plan, exec, build, config crates); UI, SCX, packaging, hostile review pending |

Nothing beyond its phase is claimed complete. The GUI lands in Phase 8; the
current binary is a foundation diagnostic that verifies the oracle freeze.

## Quick start

```sh
cargo build            # builds the workspace (no GUI yet)
cargo test             # 68 unit/property tests over the reconstructed semantics
cargo xtask oracle verify   # verifies the frozen source archive hash
cargo xtask oracle info     # prints the frozen authority record
cargo xtask court list      # lists court case directories
cargo xtask court run --all # runs courts whose fixtures are present
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
