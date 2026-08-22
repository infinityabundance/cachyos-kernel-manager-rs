# packaging/file-layout

Non-VM differential court for the Phase 10 **drop-in packaging contract**:
the candidate package's installed layout must equal the frozen oracle
package's installed surface.

- **Oracle side**: the shipped CachyOS package
  (`oracle/packages/cachyos-kernel-manager-1.19.0-1-x86_64.pkg.tar.zst`,
  sha256:3dd688c6..., hash-verified by `oracle/UPSTREAM.lock`) — its
  content-file list + the 14 shared files' sha256;
- **Candidate side**: the `packaging/` tree + the `packaging/PKGBUILD`
  install paths (the icons materialized into the hicolor `apps/` dirs
  exactly as the PKGBUILD installs them).

The 15-file list must match exactly (the binary + 2 helpers + desktop +
polkit + 10 icons), and the 14 shared drop-in files (everything except the
binary) must be byte-identical — the binary is the replacement (the Rust
GUI vs the Qt one). The identity surfaces (org names, the single-instance
lock key, the cache paths) are part of the drop-in contract
(docs/COMPATIBILITY.md, courted elsewhere).

Status: defined. Run:

```
tools/run-packaging-corpus.sh
cargo xtask court run packaging/file-layout
```

Falsifier: any file in one list missing from the other, or any shared-file
hash difference.
