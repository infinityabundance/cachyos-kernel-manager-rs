#!/usr/bin/env bash
#
# build-base.sh — HOST-side orchestrator for the VM base image.
#
#  1. generates (or reuses) the harness SSH key,
#  2. runs the privileged Docker builder container (archlinux:latest) with
#     the provisioning script, the frozen oracle tarball, and the in-VM
#     harness scripts,
#  3. copies every output OUT of the container with `docker cp` (which
#     writes as the host user — no root-owned files in vm/images),
#  4. converts base.raw -> base.qcow2 on the host (qemu-img),
#  5. records the reference_image_hash into the manifest and prints it.
#
# Outputs (vm/images/):
#   base.raw          sparse raw image (16 GiB)
#   base.qcow2        immutable base for court overlays (reference hash)
#   base-rootfs/      exported rootfs DIRECTORY (loop-free fixture baking)
#   boot/             vmlinuz + initramfs for qemu direct-kernel boot
#   harness_key       SSH key for the VM harness (generated if missing)
#   manifest.json     package manifest + hashes
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VM="$ROOT/vm"
IMAGES="$VM/images"
CONTAINER_IMAGE="${KM_BUILDER_IMAGE:-archlinux:latest}"
CONTAINER_NAME="cachyos-km-builder"

mkdir -p "$IMAGES"

step() { echo; echo "== [$(date -u +%H:%M:%S)] $*"; }

# --- harness ssh key ---
if [ ! -f "$IMAGES/harness_key" ]; then
    step "generating harness ssh key"
    ssh-keygen -q -t ed25519 -N '' -f "$IMAGES/harness_key"
fi
SSH_PUBKEY="$(cat "$IMAGES/harness_key.pub")"

# --- frozen oracle source tarball ---
TARBALL="$ROOT/oracle/upstream-v1.19.0-6b4a373.tar.gz"
[ -f "$TARBALL" ] || { echo "missing oracle tarball: $TARBALL" >&2; exit 1; }

# --- docker builder ---
step "pulling builder image ($CONTAINER_IMAGE)"
docker pull -q "$CONTAINER_IMAGE"

step "starting privileged builder container"
docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
# named cache volume survives container removal (fast restarts)
docker volume create cachyos-km-pacman-cache >/dev/null 2>&1 || true
docker run -d --name "$CONTAINER_NAME" --privileged \
    -v "cachyos-km-pacman-cache:/var/cache/pacman/pkg" \
    -v "$IMAGES:/host-images" \
    -v "$ROOT/oracle:/in/oracle:ro" \
    -v "$TARBALL:/in/oracle-src.tar.gz:ro" \
    -v "$VM/in-vm:/in/vm-in-vm:ro" \
    --memory="${KM_BUILDER_MEM_LIMIT:-16g}" --memory-swap="${KM_BUILDER_MEM_SWAP:-20g}" \
    --pids-limit=8192 \
    "$CONTAINER_IMAGE" sleep infinity >/dev/null

trap 'docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true' EXIT

# stale outputs from interrupted runs may be root-owned and unremovable by
# the host user; the container (root) removes them FIRST, BEFORE the
# provision writes fresh outputs directly to /host-images (the stale
# cleanup must NEVER run after the provision — it would delete the fresh
# base.raw/base-rootfs/boot/fixtures it just wrote)
for stale in base.raw base.qcow2 manifest.json provision.log base-rootfs boot fixtures; do
    [ -n "$stale" ] || continue
    docker exec "$CONTAINER_NAME" rm -rf "/host-images/$stale" || true
done

step "provision rootfs + oracle"
docker cp "$VM/base/provision-rootfs.sh" "$CONTAINER_NAME:/provision.sh"
docker cp "$VM/base/pacman.conf" "$CONTAINER_NAME:/etc/pacman.conf"
docker exec -e SSH_PUBKEY="$SSH_PUBKEY" -e BUILDER_IMAGE="$CONTAINER_IMAGE" \
    "$CONTAINER_NAME" bash /provision.sh

step "copying outputs out of the container (host-user ownership)"
# base.raw + rootfs.img + the base-rootfs export are written DIRECTLY to
# /host-images by the provision script (the container's tmpfs layer is too
# small for them); only small metadata is docker cp'd
docker cp "$CONTAINER_NAME:/out/manifest.json" "$IMAGES/manifest.json"
docker cp "$CONTAINER_NAME:/out/provision.log" "$IMAGES/provision.log"
docker cp "$CONTAINER_NAME:/out/boot" "$IMAGES/boot"
chmod 644 "$IMAGES/manifest.json" "$IMAGES/provision.log" 2>/dev/null || true

# --- convert to qcow2 on the host ---
step "converting base.raw -> base.qcow2"
chmod 644 "$IMAGES/base.raw" 2>/dev/null || true
qemu-img convert -f raw -O qcow2 "$IMAGES/base.raw" "$IMAGES/base.qcow2"
chmod 644 "$IMAGES/base.qcow2"
# the raw is a root-owned intermediate; remove it as root while the
# container is still alive (the qcow2 + manifest are the evidence)
docker exec "$CONTAINER_NAME" rm -f /host-images/base.raw
chmod 644 "$IMAGES/manifest.json" "$IMAGES/provision.log"

step "recording reference_image_hash"
QCOW_HASH="$(sha256sum "$IMAGES/base.qcow2" | awk '{print $1}')"
# merge the qcow2 hash into the container-written manifest
python3 - "$IMAGES/manifest.json" "$QCOW_HASH" <<'PYEOF'
import json, sys
path, qhash = sys.argv[1], sys.argv[2]
m = json.load(open(path))
m["reference_image_hash"] = f"sha256:{qhash}"
json.dump(m, open(path, "w"), indent=1)
print(f"reference_image_hash = sha256:{qhash}")
PYEOF
chmod 644 "$IMAGES/manifest.json"

step "BASE IMAGE COMPLETE"
echo "  base.qcow2 : $IMAGES/base.qcow2"
echo "  boot       : $IMAGES/boot/ ($(ls "$IMAGES"/boot/ | tr '\n' ' '))"
echo "  manifest   : $IMAGES/manifest.json"
