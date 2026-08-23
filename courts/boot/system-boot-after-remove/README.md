# boot/system-boot-after-remove

Phase 11 differential VM court: removing a NON-RUNNING real kernel + a
REBOOT. Uses the same `kernel-install` fixture as the install court (the
base kernel running; the real linux-cachyos-lts packages cached).

The in-VM sequence (both boots identical):

1. **setup** — install the cached lts (the two-kernel state);
2. **remove** (`oracle|candidate-remove-boot.sh`) — `pacman -Rsn
   linux-cachyos-lts linux-cachyos-lts-headers` under strace: the oracle
   side uses the frozen source's literal command, the candidate side uses
   the exec crate's model render; the post-remove hooks (mkinitcpio)
   remove the lts initramfs;
3. **reboot** (the runner re-boots the SAME overlay) — `boot-check-remove
   .sh` hard-asserts the system boots, the base kernel is intact, and the
   lts kernel + its initramfs are GONE.

Every written surface (remove command, exec chain, pre/post kernel + /boot
states, hook output, boot status, running kernel, machine residual) must
match byte-for-byte between the two boots.

Status: **sealed — PASS** (2026-08-23; oracle == candidate on fixture
`kernel-install`, zero residuals). Execution:

```
cargo xtask vm bake kernel-install
cargo build -p cachyos-kernel-manager-exec --bin cachyos-kernel-manager-installcmd
cargo xtask court run boot/system-boot-after-remove --vm
```

Falsifier: any remove-command/exec-chain difference, any /boot or
kernel-state difference, any boot-check failure (the base kernel missing
or the lts surviving), or any byte difference between the two boots.
