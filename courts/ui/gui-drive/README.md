# ui/gui-drive

Phase 12 production-integration slice (the audit's "drive the installed
binary" requirement): the court participant is the packaged GUI itself,
not a witness CLI.

The runner:

1. stages the release binary (`target/release/cachyos-kernel-manager`,
   built with `--features gui-alpm`) into the 9p share (`share/gui/`);
2. boots a fresh overlay of the `gui-integration` fixture (the winit X11
   client stack + at-spi2-core + ttf-dejavu) per side;
3. **oracle side**: `oracle-gui-drive.sh` launches the frozen Qt binary
   under Xvfb + strace and drives it (AT-SPI);
4. **candidate side**: `candidate-gui-drive.sh` launches the RELEASE Slint
   binary (SLINT_BACKEND=winit-software, explicit) the same way;
5. both run the SAME side-agnostic driver (`candidate-drive.py`): sort by
   every column header, toggle the first row after each sort, dump the
   full tree after every step;
6. compares `drive-semantic.json` (the sorted pkgname orders + the toggle
   identity proof) byte-for-byte + the machine residuals.

## The identity proof

After sorting by header H, the driver toggles the FIRST row's checkbox and
records which pkgname is now checked. The claim: that pkgname must equal
the first pkgname of the H-sorted order — the toggle followed the kernel's
IDENTITY through the reorder. This is the regression the stable-raw-
identity `KernelToggled { raw }` design exists to prevent (the old iced
port passed the sorted presentation index straight into the core, so
clicking visible row 0 toggled the wrong kernel).

## Run

```sh
cargo build --release --features gui-alpm
cargo xtask court run ui/gui-drive --vm
```

## Falsifiers

- any difference in the sorted pkgname order for any header between the
  two boots;
- any difference in a toggle's checked-state transition;
- the identity proof failing on either side;
- any machine-residual difference.
