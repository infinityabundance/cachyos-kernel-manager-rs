# boot/system-boot-after-install

Phase 11 differential VM court: a REAL kernel install + a REBOOT. The
fixture (`vm/fixtures/kernel-install`) runs the base kernel
(linux-cachyos 7.1.8-1) with the real linux-cachyos-lts packages CACHED
(downloaded at bake time — the court VMs are offline).

The in-VM sequence (both boots identical):

1. **install** (`oracle|candidate-boot-install.sh`) — `pacman -S --needed
   linux-cachyos-lts linux-cachyos-lts-headers` under strace: the oracle
   side uses the frozen source's literal command, the candidate side uses
   the exec crate's model render (`cachyos-kernel-manager-installcmd`);
   the post-install hooks (mkinitcpio) run and regenerate /boot;
2. **reboot** (the runner re-boots the SAME overlay — the install
   persisted) — `boot-check.sh` verifies the system comes up, the lts
   kernel is installed, its initramfs exists in /boot, and the boot
   journal is clean.

Every written surface (install command, exec chain, pre/post kernel + /boot
states, hook output, boot status, running kernel, machine residual) must
match byte-for-byte between the two boots.

Status: defined. Execution:

```
cargo xtask vm bake kernel-install
cargo build -p cachyos-kernel-manager-exec --bin cachyos-kernel-manager-installcmd
cargo xtask court run boot/system-boot-after-install --vm
```

Falsifier: any install-command/exec-chain difference, any /boot or
kernel-state difference, any boot-check failure, or any byte difference
between the two boots.
