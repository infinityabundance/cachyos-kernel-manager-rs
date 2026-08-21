# vm/

Disposable VM orchestration for oracle differential courts (Phase 2).

Architecture (directive §40): Docker/Podman orchestrates QEMU/KVM snapshot
VMs carrying the frozen oracle application and the candidate; each court
restores an identical snapshot before the candidate run.

Safety invariant (implemented in code, not just documented): destructive or
privileged courts fail closed unless the machine presents the expected
machine-id class, fixture marker, snapshot identity, and test root.

Commands (Phase 2): `cargo xtask vm build`, `cargo xtask court run <case>`.

Fixture matrix to define: minimal state, several installed kernels, upgrade/
downgrade, ZFS root, NVIDIA variants, sched_ext present/absent, custom repo,
AUR on/off, stale DB, read-only paths, low disk, offline, non-English,
Wayland/X11 (directive §41).
