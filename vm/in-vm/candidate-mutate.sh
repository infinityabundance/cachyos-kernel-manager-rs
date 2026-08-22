#!/usr/bin/env bash
#
# candidate-mutate.sh — run the CANDIDATE mutation model in the court VM and
# capture its modeled view of the SAME Configure-window mutations the oracle
# just performed.
#
# Produces (into $1, default /mnt/host/out):
#   candidate-state.json              discovery rows (inspect dump)
#   candidate-pkgbuild-before.txt     the fixture PKGBUILD (fresh overlay)
#   candidate-pkgbuild-after.txt      the modeled mutation
#   candidate.stderr                  model tool stderr
#   residual.json                     machine residual (must equal the oracle's)
#
# Usage: candidate-mutate.sh <out-dir> [--custom-name <n>] [--patch-url <u>]
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
shift || true
ACTIONS=("$@")

mkdir -p "$OUT"

# --- iterate without rebaking: prefer the share copy when present ---
if [ -f /mnt/host/scripts/candidate-mutate.sh ] && [ "$0" != "/mnt/host/scripts/candidate-mutate.sh" ]; then
    exec /mnt/host/scripts/candidate-mutate.sh "$OUT" "${ACTIONS[@]}"
fi

# --- safety gate ---
if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi

INSPECT=/opt/cachyos-km-vm/cachyos-kernel-manager-inspect
[ -x "$INSPECT" ] || INSPECT=/mnt/host/inspect/cachyos-kernel-manager-inspect
if [ ! -x "$INSPECT" ]; then
    echo "FATAL: candidate inspect tool not found" >&2
    exit 4
fi
MUTATE=/opt/cachyos-km-vm/cachyos-kernel-manager-mutate
[ -x "$MUTATE" ] || MUTATE=/mnt/host/inspect/cachyos-kernel-manager-mutate
if [ ! -x "$MUTATE" ]; then
    echo "FATAL: candidate mutation model tool not found" >&2
    exit 4
fi

# parse --custom-name / --patch-url (same argv the oracle side received);
# an empty custom name means the window default ($pkgbase-custom)
CUSTOM_NAME='$pkgbase-custom'
PATCH_URL=""
i=0
while [ $i -lt ${#ACTIONS[@]} ]; do
    case "${ACTIONS[$i]}" in
        --custom-name) CUSTOM_NAME="${ACTIONS[$((i+1))]}" ;;
        --patch-url) PATCH_URL="${ACTIONS[$((i+1))]}" ;;
    esac
    i=$((i + 2))
done
if [ -z "$CUSTOM_NAME" ]; then
    CUSTOM_NAME='$pkgbase-custom'
fi

# machine residual: must be IDENTICAL to the oracle run's (same fixture,
# fresh overlay) — the comparator treats drift as a fixture-integrity
# violation, not a parity pass.
RESIDUAL_SH="/opt/cachyos-km-vm/residual.sh"
[ -f /mnt/host/scripts/residual.sh ] && RESIDUAL_SH=/mnt/host/scripts/residual.sh
"$RESIDUAL_SH" > "$OUT/residual.json" || true

"$INSPECT" dump --format json > "$OUT/candidate-state.json" 2> "$OUT/candidate.stderr"

PKGBUILD=/root/.cache/cachyos-km/pkgbuilds/linux-cachyos/PKGBUILD

# the fixture PKGBUILD from THIS fresh overlay (the oracle's pre-mutation
# text — the comparator proves both sides started from identical bytes)
cp "$PKGBUILD" "$OUT/candidate-pkgbuild-before.txt"

# the modeled mutation: source-array splice + pkgbase insert, in the
# oracle's on_execute order (conf-window.cpp:716-729)
MUTATE_ARGS=("$PKGBUILD" "$CUSTOM_NAME")
if [ -n "$PATCH_URL" ]; then
    MUTATE_ARGS+=("$PATCH_URL")
fi
"$MUTATE" "${MUTATE_ARGS[@]}" > "$OUT/candidate-pkgbuild-after.txt" 2>> "$OUT/candidate.stderr"
echo "candidate mutation observation complete (custom_name=$CUSTOM_NAME patch_url=$PATCH_URL)"
