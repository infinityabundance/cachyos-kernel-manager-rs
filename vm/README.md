# vm/ — Disposable VM oracle architecture (Phase 2)

Docker is the **orchestrator**, not the machine model. A privileged Docker
container builds and mutates images; courts run in disposable QEMU/KVM VMs
from immutable snapshots.

```
Docker controller (privileged builder container)
      ↓
vm/base/provision-rootfs.sh   pacstrap CachyOS rootfs + build frozen oracle
      ↓
base.raw → base.qcow2         immutable base (reference_image_hash in lock)
      ↓
vm/fixtures/<fixture>/spec.sh offline chroot bake → fixture.qcow2 (digest)
      ↓
qemu-img overlay (fresh copy-on-write of the fixture)
      ↓
oracle (real GUI, Xvfb + AT-SPI + strace)   → courts/…/oracle/
restore identical snapshot
      ↓
candidate (cachyos-kernel-manager-inspect)   → courts/…/candidate/
      ↓
FRF comparator → residual.json + evidence.json
```

## Layout

```
base/            pacman.conf (VM flavor) + build scripts
fixtures/        per-fixture specs (spec.sh) + lib/ helpers
harness/         vm-ctl.sh: boot/exec/put/stop via SSH (qemu user-net) + 9p share
in-vm/           scripts running inside the VM (observation, residual, safety)
images/          build artifacts (gitignored): base, fixtures, overlays, share
```

## Safety (directive §74)

Every destructive or privileged court fails closed unless the machine shows
`/etc/cachyos-km/fixture.marker` (an unmistakable VM marker) — checked by the
in-VM scripts before any observation. The host package database is never a
mutation target: package state changes happen only inside images/chroots.

## OOM protection (host safety)

A court VM must never be able to push the HOST into global memory pressure
(a 16 GiB VM on a busy host previously triggered the host OOM killer, which
killed unrelated host processes). Two layers enforce this:

1. **QEMU cgroup cap** (`vm/harness/vm-ctl.sh`): each qemu runs inside its
   own transient systemd user scope with `MemoryMax` (default 12G) and
   `MemorySwapMax` (default 4G). If a guest exceeds the cap, the kernel
   oom-kills qemu *within that cgroup* — the host is never affected. The cap
   sits well above the real footprint (an idle court VM peaks around
   0.75 GiB; qemu allocates guest RAM lazily), so it only trips on runaway
   growth.

2. **Docker builder caps** (`vm/base/build-base.sh`, `vm/fixtures/bake.sh`):
   the privileged builder containers run with `--memory=8g --memory-swap=12g
   --pids-limit=8192`.

Tuning (deliberate, documented):

```sh
KM_VM_MEM=16G KM_VM_MEM_MAX=20G cargo xtask court run …   # e.g. heavy build courts
KM_BUILDER_MEM_LIMIT=16g cargo xtask vm build               # oracle cmake builds
```

Raising the caps trades host safety for guest headroom; it is a conscious
choice for courts that genuinely need it (kernel builds), never the default.

Notes: `systemd-run` serializes its command line into a transient unit whose
ExecStart re-parsing collapses `$$` into a literal `$` — never write the
pidfile from a shell `$$` inside the scope; qemu's own `-pidfile` is used
instead. The qemu process is launched with `-device virtio-balloon-pci` so
unused guest pages can be returned to the host.

Guest boot time is host-dependent: ~10s on an idle host, but under host
swap/I-O pressure it has taken 4+ minutes (observed). `wait_ssh` polls for
up to 360s and logs progress every 30s; slow boots are environment
variance, not a harness defect.

## Determinism

CachyOS is rolling; reproducibility is snapshot-based:

- courts always run from fresh overlays of one baked fixture image,
- the fixture digest (qcow2 sha256) and full `pacman -Q` manifest are
  recorded at bake time,
- the machine residual is compared between the oracle and candidate runs:
  any drift is a fixture-integrity residual, not a parity pass.

## Fixture matrix (directive §41) — Phase 2 baseline

baked: `minimal`, `several-kernels`, `upgrade-available`, `downgrade-visible`,
`custom-repo`, `cross-repo-installed`, `duplicate-across-repos`, `stale-db`,
`empty-all-dbs`, `empty-sync-db`.

deferred to later phases (documented, marked): ZFS root (needs a fixture
`findmnt` wrapper — narrowest verifiable simulation boundary), NVIDIA
profiles (chwd wrapper), sched_ext absent (kernel feature), low disk,
offline, non-English locale, Wayland/X11, AUR on/off (requires a second
oracle build with `ENABLE_AUR_KERNELS`).

## Commands

```sh
cargo xtask vm build                 # base image (docker + qemu)
cargo xtask vm bake <fixture>        # fixture image
cargo xtask court run <case> --vm    # full differential court
cargo xtask evidence verify          # content-addressed evidence check
```
