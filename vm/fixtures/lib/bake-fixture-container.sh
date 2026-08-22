#!/usr/bin/env bash
#
# bake-fixture-container.sh — runs INSIDE the privileged Docker builder
# container (loop-free). Bakes a court fixture:
#
#   1. copy the exported base ROOTFS DIRECTORY (no images, no mounts),
#   2. apply the fixture spec in an arch-chroot (dev/proc/sys bind-mounted
#      into the chroot directory),
#   3. mkfs.ext4 -d the modified rootfs -> fs image -> dd into fixture.raw
#      at the partition offset,
#   4. capture the package manifest.
#
# Inputs (bind mounts):
#   /in/base-rootfs      exported base rootfs directory (read-only)
#   /in/fixture/         fixture spec dir (spec.sh)
#   /in/fixture-lib.sh   helper library
#   /out/                host vm/images/fixtures/<name>/
#
set -euo pipefail

LOG=/out/bake.log
exec > >(tee -a "$LOG") 2>&1

step() { echo; echo "== [$(date -u +%H:%M:%S)] $*"
}

ROOT=/work/rootfs
OUT=/out
DISK_SIZE=16G
ROOT_LABEL="cachyoskmroot"
# The container's own writable layer is tmpfs-backed (docker data root on
# tmpfs, 26G) and fills up on larger fixtures. Big image files are written
# DIRECTLY to the host bind mount instead; only small metadata goes to /out
# (docker cp'd to the host by bake.sh).
BIG_OUT="/host-images/fixtures/${FIXTURE:?FIXTURE env required}"

mkdir -p "$OUT" "$BIG_OUT"

step "builder container prerequisites"
pacman -Syu --noconfirm >/dev/null 2>&1 || true
pacman -S --noconfirm --needed arch-install-scripts util-linux e2fsprogs >/dev/null

step "copy base rootfs directory"
mkdir -p /work
rm -rf "$ROOT"
cp -a /in/base-rootfs "$ROOT"
# the exported rootfs arrived via docker cp, which rewrites ownership to
# the HOST user (uid 1000). Restore root ownership on the work copy —
# modes are already preserved by cp -a.
chown -R 0:0 "$ROOT"
chmod -R u+rwX "$ROOT"

step "stage fixture scripts into chroot"
mkdir -p "$ROOT/opt/cachyos-km-vm"
cp /in/fixture-lib.sh "$ROOT/opt/cachyos-km-vm/fixture-lib.sh"
cp /in/fixture/spec.sh "$ROOT/opt/cachyos-km-vm/fixture-spec.sh"
chmod +x "$ROOT/opt/cachyos-km-vm/fixture-spec.sh"

step "run fixture spec (arch-chroot with live bind mounts)"
mount --bind /dev "$ROOT/dev"
mount --bind /proc "$ROOT/proc"
mount --bind /sys "$ROOT/sys"
trap 'umount "$ROOT/sys" "$ROOT/proc" "$ROOT/dev" 2>/dev/null || true' EXIT
arch-chroot "$ROOT" bash /opt/cachyos-km-vm/fixture-spec.sh
umount "$ROOT/sys" "$ROOT/proc" "$ROOT/dev" 2>/dev/null || true
trap - EXIT
# flush dirty page cache left by the spec (pacman/installs): the cgroup
# memory cap counts it, and a burst of dirty pages during mkfs can OOM the
# builder even when the real working set is far below the cap
sync

step "capture package manifest"
# tolerate fixtures whose /etc/pacman.conf is deliberately malformed or
# missing (the malformed/missing-conf courts): pacman -Q then fails, but the
# manifest is evidence metadata — degrade to an empty list, never abort
arch-chroot "$ROOT" pacman -Q 2>/dev/null | sort > "$OUT/packages.txt" || true
chmod 644 "$OUT/packages.txt"

step "create fixture disk image (loop-free)"
rm -f "$BIG_OUT/fixture.raw"
truncate -s "$DISK_SIZE" "$BIG_OUT/fixture.raw"
printf 'type=83, start=2048, bootable\n' | sfdisk "$BIG_OUT/fixture.raw" >/dev/null
rm -f "$BIG_OUT/rootfs.img"
truncate -s 8G "$BIG_OUT/rootfs.img"
mkfs.ext4 -q -L "$ROOT_LABEL" -d "$ROOT" "$BIG_OUT/rootfs.img"
# write back the fs image before dd so its pages are reclaimable, not dirty
sync
dd if="$BIG_OUT/rootfs.img" of="$BIG_OUT/fixture.raw" bs=1M seek=1 conv=notrunc status=none
sync
# rootfs.img is an intermediate; drop it right away (the host disk is tight)
rm -f "$BIG_OUT/rootfs.img"
chmod 644 "$BIG_OUT/fixture.raw"

RAW_HASH="$(sha256sum "$BIG_OUT/fixture.raw" | awk '{print $1}')"
cat > "$OUT/fixture-manifest.json" <<EOF
{
  "fixture": "${FIXTURE}",
  "raw_image_hash": "sha256:$RAW_HASH",
  "baked_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
chmod 644 "$OUT/fixture-manifest.json"
echo "bake complete: $BIG_OUT/fixture.raw"
