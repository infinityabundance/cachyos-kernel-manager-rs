#!/usr/bin/env bash
#
# boot-smoke.sh — first-boot validation of the base image. Boots a fresh
# overlay, checks the essential surfaces, then powers off.
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IMAGES="$ROOT/vm/images"
CTL="$ROOT/vm/harness/vm-ctl.sh"

[ -f "$IMAGES/base.qcow2" ] || { echo "base.qcow2 missing" >&2; exit 1; }

OVERLAY="$IMAGES/overlays/smoke.qcow2"
rm -f "$OVERLAY"
qemu-img create -f qcow2 -F qcow2 -b "$IMAGES/base.qcow2" "$OVERLAY" >/dev/null

fail() { echo "SMOKE FAIL: $*" >&2; "$CTL" stop >/dev/null 2>&1 || true; exit 1; }

"$CTL" start "$OVERLAY"

echo "== 1. fixture marker (safety gate)"
"$CTL" exec "test -f /etc/cachyos-km/fixture.marker" || fail "fixture marker missing"
echo "   ok"

echo "== 2. oracle binary present + version"
"$CTL" exec "test -x /usr/bin/cachyos-kernel-manager" || fail "oracle missing"
"$CTL" exec "/usr/bin/cachyos-kernel-manager --version" || true
echo "   ok"

echo "== 3. oracle runtime deps"
"$CTL" exec "ldconfig -p | grep -E 'libscxctl-ui|libalpm|libQt6Widgets' | head -3" || fail "deps missing"
echo "   ok"

echo "== 4. Xvfb + a11y client (vendored pyatspi2)"
"$CTL" exec "which Xvfb" >/dev/null || fail "Xvfb missing"
"$CTL" exec "test -d /opt/a11y/pyatspi2/pyatspi" || fail "pyatspi2 missing"
"$CTL" exec "python3 -c 'import gi.repository.Atspi'" || fail "gi Atspi typelib missing"
echo "   ok"

echo "== 5. pacman state + kernel"
"$CTL" exec "pacman -Q linux-cachyos linux-cachyos-headers" || fail "kernel not installed"
echo "   ok"

echo "== 6. sync databases registered"
"$CTL" exec "ls /var/lib/pacman/sync/" || fail "no sync dbs"
echo "   ok"

"$CTL" stop
echo "SMOKE PASS"
