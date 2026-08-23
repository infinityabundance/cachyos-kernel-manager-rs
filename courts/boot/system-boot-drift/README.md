# boot/system-boot-drift

Phase 11 differential VM court: the multi-reboot drift witness — a REAL
kernel install followed by THREE reboots of the SAME overlay. Uses the
same `kernel-install` fixture as the install/remove/failure courts (the
base kernel running; the real linux-cachyos-lts packages cached).

The in-VM sequence (both boots identical):

1. **install** (`oracle|candidate-boot-install.sh`) — `pacman -S --needed
   linux-cachyos-lts linux-cachyos-lts-headers` under strace: the oracle
   side uses the frozen source's literal command, the candidate side uses
   the exec crate's model render; the post-install hooks (mkinitcpio)
   regenerate /boot with the lts initramfs;
2. **reboot × 3** (the runner re-boots the SAME overlay three times) —
   `boot-check-drift.sh <out> <N>` records a suffixed surface after each
   reboot (boot-status-$N.txt, running-kernel-$N.txt, kernels-$N.txt,
   boot-files-$N.txt, journal-tail-$N.txt), hard-asserting the machine
   came up and the install persisted.

Every written surface (install command, exec chain, pre/post kernel +
/boot states, hook output, and the three reboot surfaces) must be
byte-identical: the three reboot surfaces must not drift from each other
on either side, and both sides must match byte-for-byte.

Status: **sealed — PASS** (2026-08-23; oracle == candidate on fixture
`kernel-install`, zero residuals). Execution:

```
cargo xtask vm bake kernel-install
cargo build -p cachyos-kernel-manager-exec --bin cachyos-kernel-manager-installcmd
cargo xtask court run boot/system-boot-drift --vm
```

Falsifier: any install-command/exec-chain difference, any /boot or
kernel-state difference, any boot-check failure, any drift between the
three reboot surfaces on a side, or any byte difference between the two
sides.
