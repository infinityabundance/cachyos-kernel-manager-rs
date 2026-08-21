#!/usr/bin/env bash
#
# bake.sh <fixture> — HOST-side fixture baker.
#
#  1. runs the container-side baker (privileged docker, archlinux:latest)
#     against the exported base rootfs directory (loop-free),
#  2. copies the outputs out with `docker cp` (host-user ownership),
#  3. converts fixture.raw -> fixture.qcow2 (host qemu-img),
#  4. records the fixture digest (qcow2 sha256) + spec hash into
#     vm/images/fixtures/<name>/fixture-manifest.json.
#
set -euo pipefail

FIXTURE="${1:?usage: bake.sh <fixture>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VM="$ROOT/vm"
IMAGES="$VM/images"
CONTAINER_IMAGE="${KM_BUILDER_IMAGE:-archlinux:latest}"
# mkfs.ext4 -d on the full rootfs tree is memory-hungry (it builds the
# inode table in RAM); 8g was observed to OOM the builder on larger
# fixtures. 16g RAM + generous swap headroom keeps the host safe (the
# cgroup OOM-kills only the builder, never the host).
BUILDER_MEM="${KM_BUILDER_MEM_LIMIT:-16g}"
BUILDER_SWAP="${KM_BUILDER_MEM_SWAP:-32g}"

SPEC="$VM/fixtures/$FIXTURE"
[ -f "$SPEC/spec.sh" ] || { echo "no such fixture: $FIXTURE ($SPEC/spec.sh)" >&2; exit 1; }
[ -d "$IMAGES/base-rootfs" ] || { echo "base-rootfs missing — run: cargo xtask vm build" >&2; exit 1; }

OUT="$IMAGES/fixtures/$FIXTURE"
mkdir -p "$OUT"

echo "== baking fixture: $FIXTURE"
docker pull -q "$CONTAINER_IMAGE" >/dev/null 2>&1 || true

CID="cachyos-km-bake"
docker rm -f "$CID" >/dev/null 2>&1 || true
docker run -d --name "$CID" --privileged \
    -v "cachyos-km-pacman-cache:/var/cache/pacman/pkg" \
    -v "$IMAGES:/host-images" \
    -v "$IMAGES/base-rootfs:/in/base-rootfs:ro" \
    -v "$SPEC:/in/fixture:ro" \
    -v "$VM/fixtures/lib/fixture-lib.sh:/in/fixture-lib.sh:ro" \
    -v "$VM/fixtures/lib/bake-fixture-container.sh:/in/bake.sh:ro" \
    --memory="$BUILDER_MEM" --memory-swap="$BUILDER_SWAP" \
    --pids-limit=8192 \
    "$CONTAINER_IMAGE" sleep infinity >/dev/null
trap 'docker rm -f "$CID" >/dev/null 2>&1 || true' EXIT

# stale outputs from interrupted runs may be root-owned and unremovable by
# the host user; the container (root) removes them first
docker exec "$CID" rm -rf "/host-images/fixtures/$FIXTURE" || true
mkdir -p "$OUT"

docker exec -e FIXTURE="$FIXTURE" "$CID" bash /in/bake.sh

# the big image files are written directly to /host-images (the container's
# tmpfs-backed layer is too small); only small metadata is docker cp'd
chmod 644 "$OUT/fixture.raw" 2>/dev/null || true

echo "== copying metadata (host-user ownership)"
docker cp "$CID:/out/packages.txt" "$OUT/packages.txt"
docker cp "$CID:/out/fixture-manifest.json" "$OUT/fixture-manifest.json"
docker cp "$CID:/out/bake.log" "$OUT/bake.log" 2>/dev/null || true

echo "== converting fixture.raw -> fixture.qcow2"
qemu-img convert -f raw -O qcow2 "$OUT/fixture.raw" "$OUT/fixture.qcow2"
chmod 644 "$OUT/fixture.qcow2" "$OUT/packages.txt" "$OUT/fixture-manifest.json"
# the raw image is an intermediate; the qcow2 + digest are the evidence
# (the host disk is tight — 10 fixtures × ~4G raw would otherwise linger).
# The raw is root-owned (the container wrote it), so remove it as root via
# docker exec while the container is still alive.
docker exec "$CID" rm -f "/host-images/fixtures/$FIXTURE/fixture.raw"

FIXTURE_HASH="$(sha256sum "$OUT/fixture.qcow2" | awk '{print $1}')"
SPEC_HASH="$(sha256sum "$SPEC/spec.sh" | awk '{print $1}')"
python3 - "$OUT/fixture-manifest.json" "$FIXTURE_HASH" "$SPEC_HASH" <<'PYEOF'
import json, sys
path, fhash, shash = sys.argv[1], sys.argv[2], sys.argv[3]
m = json.load(open(path))
m["fixture_digest"] = f"sha256:{fhash}"
m["spec_sha256"] = f"sha256:{shash}"
json.dump(m, open(path, "w"), indent=1)
print(f"fixture_digest = sha256:{fhash}")
PYEOF
echo "fixture $FIXTURE baked: $OUT/fixture.qcow2"
