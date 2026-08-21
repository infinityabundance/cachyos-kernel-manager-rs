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

mkdir -p "$OUT"/

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

step "capture package manifest"
arch-chroot "$ROOT" pacman -Q | sort > "$OUT/packages.txt"
chmod 644 "$OUT/packages.txt"

step "create fixture disk image (loop-free)"
rm -f "$OUT/fixture.raw"
truncate -s "$DISK_SIZE" "$OUT/fixture.raw"
printf 'type=83, start=2048, bootable\n' | sfdisk "$OUT/fixture.raw" >/dev/null
rm -f "$OUT/rootfs.img"
truncate -s 8G "$OUT/rootfs.img"
mkfs.ext4 -q -L "$ROOT_LABEL" -d "$ROOT" "$OUT/rootfs.img"
dd if="$OUT/rootfs.img" of="$OUT/fixture.raw" bs=1M seek=1 conv=notrunc status=none
sync

RAW_HASH="$(sha256sum "$OUT/fixture.raw" | awk '{print $1}')"
cat > "$OUT/fixture-manifest.json" <<EOF
{
  "fixture": "$(basename "$OUT")",
  "raw_image_hash": "sha256:$RAW_HASH",
  "baked_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
chmod 644 "$OUT/fixture-manifest.json"
echo "bake complete: $OUT/fixture.raw"
