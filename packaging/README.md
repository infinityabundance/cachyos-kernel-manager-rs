# packaging/

Drop-in Arch/CachyOS packaging (Phase 10):

- `PKGBUILD` — builds the Rust GUI (`cargo build --release --features
  gui-alpm --locked`) and installs the drop-in surface: the binary at
  `/usr/bin/cachyos-kernel-manager`, the two privileged helpers, the
  desktop entry, the polkit policy, and the ten hicolor icons — all
  byte-identical to the frozen oracle package (courted by
  `packaging/file-layout`).
- `usr/` — the packaged drop-in files (the shared surface).
- `tools/build-candidate-package.sh` — assembles the candidate
  `.pkg.tar.zst` from the local tree exactly as the PKGBUILD installs it
  (the packaging/upgrade court's artifact).
- `oracle/packages/` (repo root) — the frozen oracle package
  (`cachyos-kernel-manager-1.19.0-1-x86_64.pkg.tar.zst`, sha256
  `3dd688c6...`, hash-verified by `oracle/UPSTREAM.lock`): the revert
  target + the file-layout authority.

Courts:

- `packaging/file-layout` (pure) — the candidate's installed file set ==
  the oracle package's 15-file surface; the 14 shared files byte-identical.
- `packaging/upgrade` (VM) — oracle → candidate → oracle transition:
  `pacman -U` replaces the package, the file surface + the discovery rows
  are preserved, `--version` reports the Rust 0.1.0 (the oracle has no
  `--version` handling — it aborts; documented in
  docs/KNOWN_DIVERGENCES.md), the revert restores 1.19.0-1.

Build + verify locally:

```sh
tools/build-candidate-package.sh
tools/run-packaging-corpus.sh && cargo xtask court run packaging/file-layout
cargo xtask court run packaging/upgrade --vm
```
