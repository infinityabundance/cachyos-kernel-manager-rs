# boot/system-boot-after-failure

Phase 11 differential VM court: the FAILED-boot residual comparison —
removing the RUNNING kernel + a REBOOT. Uses the same `kernel-install`
fixture as the install/remove courts (the base kernel running; the real
linux-cachyos-lts packages cached).

The in-VM sequence (both boots identical):

1. **setup** — install the cached lts (the two-kernel state);
2. **remove** (`oracle|candidate-fail-boot.sh`) — `pacman -Rsn
   linux-cachyos linux-cachyos-headers` under strace: the oracle side uses
   the frozen source's literal command, the candidate side uses the exec
   crate's model render; the post-remove hooks (mkinitcpio) remove the
   base initramfs;
3. **reboot attempt** (the runner re-boots the SAME overlay with a bounded
   ssh probe) — the machine does NOT become usable after the running
   kernel's removal (no ssh within 240s): `boot-attempt.txt` records the
   no-ssh failure witness, byte-identical on both sides. (A boot that DID
   come up is still valid — `boot-check-failure.sh` then hard-asserts the
   failed state: the base kernel + its /boot entry GONE, the lts + its
   initramfs remaining.)

Every written surface (remove command, exec chain, pre/post kernel + /boot
states, hook output, boot-attempt.txt, and — only when a boot came up —
the boot-check surfaces) must match byte-for-byte between the two boots.
The boot-check assertions are INVERTED vs the remove court: they hard-fail
unless the failure residual is exactly present.

Status: **sealed — PASS** (2026-08-23; oracle == candidate on fixture
`kernel-install`, zero residuals). Execution:

```
cargo xtask vm bake kernel-install
cargo build -p cachyos-kernel-manager-exec --bin cachyos-kernel-manager-installcmd
cargo xtask court run boot/system-boot-after-failure --vm
```

Falsifier: any remove-command/exec-chain difference, any /boot or
kernel-state difference, any boot-check failure (the base kernel
surviving or the lts missing), or any byte difference between the two
boots.
