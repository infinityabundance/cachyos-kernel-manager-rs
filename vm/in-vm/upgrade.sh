#!/usr/bin/env bash
#
# upgrade.sh — the packaging/upgrade court's in-VM script (Phase 10). Runs
# the SAME sequence on both boots:
#
#   1. BASELINE (the oracle package installed): `pacman -Q` version, the
#      file list, `--version`, the discovery row names (the oracle GUI via
#      AT-SPI);
#   2. UPGRADE: `pacman -U` the CANDIDATE package (same pkgname -> the
#      oracle package is replaced); verify the version (0.1.0-1), the file
#      list (== baseline), `--version` (the Rust binary), the discovery row
#      names (the inspect tool on the same dbs);
#   3. REVERT: `pacman -U` the frozen oracle package back; verify the
#      version (1.19.0-1) + `--version` (the Qt binary).
#
# The transition assertions are HARD failures (a broken drop-in aborts the
# side). The written files are compared byte-for-byte between the boots.
#
set -euo pipefail
OUT="$1"
mkdir -p "$OUT"

PKGS_DIR=/mnt/host/packaging
CAND_PKG="$PKGS_DIR/cachyos-kernel-manager-0.1.0-1-x86_64.pkg.tar.zst"
ORACLE_PKG="$PKGS_DIR/cachyos-kernel-manager-1.19.0-1-x86_64.pkg.tar.zst"

# ---- 0. baseline: install the FROZEN oracle PACKAGE (the base image
# builds the oracle from source; the package is the frozen authority). The
# cmake-installed files conflict -> --overwrite the package's own paths. ----
pacman -U --noconfirm \
    --overwrite '/usr/bin/cachyos-kernel-manager' \
    --overwrite '/usr/lib/cachyos-kernel-manager/*' \
    --overwrite '/usr/share/applications/org.cachyos.KernelManager.desktop' \
    --overwrite '/usr/share/polkit-1/actions/*' \
    --overwrite '/usr/share/icons/hicolor/*/apps/*' \
    "$ORACLE_PKG" >/tmp/baseline-install.log 2>&1 \
    || { cat /tmp/baseline-install.log >&2; exit 1; }

# ---- 1. baseline (the oracle package) ----
pacman -Q cachyos-kernel-manager | cut -d' ' -f2 > "$OUT/baseline-version.txt"
[ "$(cat "$OUT/baseline-version.txt")" = "1.19.0-1" ] || { echo "BASELINE: not the oracle 1.19.0-1" >&2; exit 1; }
pacman -Ql cachyos-kernel-manager | awk '{print $2}' | grep -v '/$' | sort \
    > "$OUT/baseline-filelist.txt"
/usr/bin/cachyos-kernel-manager --version > "$OUT/baseline-version-flag.txt" 2>&1 || true

# the oracle GUI's discovery rows (AT-SPI), into a scratch dir
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
bash /opt/cachyos-km-vm/oracle-observe.sh "$SCRATCH" >/dev/null 2>&1 || true
if [ -f "$SCRATCH/oracle-state.json" ]; then
    python3 - <<'EOF' "$SCRATCH/oracle-state.json" > "$OUT/baseline-rows.txt"
import json,sys
d = json.load(open(sys.argv[1]))
rows = d.get("rows", [])
print("\n".join(sorted(r.get("raw","") for r in rows)))
EOF
else
    echo "BASELINE: oracle observation failed" >&2
    exit 1
fi

# ---- 2. upgrade: the oracle package -> the candidate package ----
pacman -U --noconfirm "$CAND_PKG" >/tmp/upgrade.log 2>&1 || { cat /tmp/upgrade.log >&2; exit 1; }
pacman -Q cachyos-kernel-manager | cut -d' ' -f2 > "$OUT/upgraded-version.txt"
[ "$(cat "$OUT/upgraded-version.txt")" = "0.1.0-1" ] || { echo "UPGRADE: not the candidate 0.1.0-1" >&2; exit 1; }
pacman -Ql cachyos-kernel-manager | awk '{print $2}' | grep -v '/$' | sort \
    > "$OUT/upgraded-filelist.txt"
# the drop-in surface: the file list must be UNCHANGED by the transition
diff -u "$OUT/baseline-filelist.txt" "$OUT/upgraded-filelist.txt" >/dev/null \
    || { echo "UPGRADE: the file surface changed" >&2; exit 1; }
/usr/bin/cachyos-kernel-manager --version > "$OUT/upgraded-version-flag.txt" 2>&1 || true
grep -q "cachyos-kernel-manager 0.1.0" "$OUT/upgraded-version-flag.txt" \
    || { echo "UPGRADE: --version is not the Rust binary" >&2; exit 1; }

# the candidate's discovery rows (the inspect tool on the SAME dbs)
/mnt/host/inspect/cachyos-kernel-manager-inspect dump --json 2>/dev/null \
    | python3 -c "
import json,sys
d = json.load(sys.stdin)
print('\n'.join(sorted(r.get('raw','') for r in d.get('rows', []))))" \
    > "$OUT/upgraded-rows.txt"
# the discovery surface: the same kernels before and after the transition
diff -u "$OUT/baseline-rows.txt" "$OUT/upgraded-rows.txt" >/dev/null \
    || { echo "UPGRADE: the discovery rows changed" >&2; exit 1; }

# ---- 3. revert: the candidate package -> the oracle package ----
pacman -U --noconfirm "$ORACLE_PKG" >/tmp/revert.log 2>&1 || { cat /tmp/revert.log >&2; exit 1; }
pacman -Q cachyos-kernel-manager | cut -d' ' -f2 > "$OUT/reverted-version.txt"
[ "$(cat "$OUT/reverted-version.txt")" = "1.19.0-1" ] || { echo "REVERT: not the oracle 1.19.0-1" >&2; exit 1; }
/usr/bin/cachyos-kernel-manager --version > "$OUT/reverted-version-flag.txt" 2>&1 || true

pacman -Q > "$OUT/packages.txt" 2>/dev/null || true
