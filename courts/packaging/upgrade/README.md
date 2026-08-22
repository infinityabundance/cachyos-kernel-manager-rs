# packaging/upgrade

Differential VM court for the Phase 10 **package transition**: the
oracle-package → candidate-package → oracle-package cycle, witnessed on the
real system with real pacman.

The in-VM script (`vm/in-vm/upgrade.sh`) runs the SAME sequence on both
boots:

1. **baseline** — the frozen oracle package (`cachyos-kernel-manager`
   1.19.0-1, sha256:3dd688c6...): the version, the file list, `--version`,
   the discovery rows (the real Qt GUI via AT-SPI);
2. **upgrade** — `pacman -U` the candidate package (0.1.0-1, built by
   `tools/build-candidate-package.sh` from the local tree exactly as
   `packaging/PKGBUILD` installs it): the package is REPLACED; the file
   list must be **unchanged** (the drop-in surface), `--version` must be
   the Rust binary, and the discovery rows (the inspect tool on the same
   dbs) must equal the baseline's;
3. **revert** — `pacman -U` the frozen oracle package back: 1.19.0-1 and
   the Qt binary restored.

Every assertion hard-fails the side (a broken transition aborts). All
written surfaces must match byte-for-byte between the two boots (the
transition is deterministic and stable).

Status: defined. Execution:

```
tools/build-candidate-package.sh
cargo xtask court run packaging/upgrade --vm
```

Falsifier: any file-surface change, any discovery-row change, any broken
transition assertion, or any byte difference between the two boots.
