#!/usr/bin/env bash
#
# run-packaging-corpus.sh — witness runner for the packaging/file-layout
# court (Phase 10).
#
# Compares the CANDIDATE package's installed layout (the PKGBUILD install
# paths + the packaging/ tree) against the FROZEN ORACLE PACKAGE's installed
# file set:
#   oracle/files.txt              the oracle package's file list
#   oracle/shared-files.sha256    sha256 of the 14 shared drop-in files
#                                 (helpers, desktop, polkit, icons)
#   candidate/files.txt           the PKGBUILD's install file list
#   candidate/shared-files.sha256 sha256 of the SAME 14 files in packaging/
#
# The file SETS must match and the shared files must be byte-identical (the
# binary is the REPLACEMENT — its hash intentionally differs).
#
# Then: cargo xtask court run packaging/file-layout
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/packaging/file-layout"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"
PKG="$ROOT/oracle/packages/cachyos-kernel-manager-1.19.0-1-x86_64.pkg.tar.zst"

mkdir -p "$ORACLE" "$CANDIDATE"

# the oracle package's file list (content files only)
bsdtar -tf "$PKG" \
    | grep -vE '^\.(BUILDINFO|MTREE|PKGINFO)$' \
    | grep -vE '/$' | sort > "$ORACLE/files.txt"

# the candidate's install file list (the PKGBUILD install paths)
{
    echo "usr/bin/cachyos-kernel-manager"
    echo "usr/lib/cachyos-kernel-manager/terminal-helper"
    echo "usr/lib/cachyos-kernel-manager/rootshell.sh"
    echo "usr/share/applications/org.cachyos.KernelManager.desktop"
    echo "usr/share/polkit-1/actions/org.cachyos.KernelManager.pkexec.policy"
    for d in 16x16 22x22 32x32 44x44 48x48 64x64 128x128 150x150 256x256 310x310; do
        echo "usr/share/icons/hicolor/$d/apps/org.cachyos.KernelManager.png"
    done
} | sort > "$CANDIDATE/files.txt"

# the shared files' hashes (both sides)
extract_shared_hashes() { # extract_shared_hashes <base> <out>
    local base="$1" out="$2"
    {
        sha256sum "$base/usr/lib/cachyos-kernel-manager/terminal-helper" | cut -d' ' -f1
        sha256sum "$base/usr/lib/cachyos-kernel-manager/rootshell.sh" | cut -d' ' -f1
        sha256sum "$base/usr/share/applications/org.cachyos.KernelManager.desktop" | cut -d' ' -f1
        sha256sum "$base/usr/share/polkit-1/actions/org.cachyos.KernelManager.pkexec.policy" | cut -d' ' -f1
        for d in 16x16 22x22 32x32 44x44 48x48 64x64 128x128 150x150 256x256 310x310; do
            sha256sum "$base/usr/share/icons/hicolor/$d/apps/org.cachyos.KernelManager.png" | cut -d' ' -f1
        done
    } > "$out"
}

# The same hashes with the desktop entry NORMALIZED (D-007): the candidate's
# desktop file deliberately adds the StartupWMClass line + its explanatory
# comment (the Qt oracle got its taskbar grouping from Qt's WM_CLASS; winit
# windows need StartupWMClass to restore it). The normalizer strips exactly
# those lines so the normalized hashes prove the desktop entry differs from
# the frozen oracle ONLY by the documented adaptation.
# normalizer: desktop-startupwmclass-strip v1
extract_shared_hashes_norm() { # extract_shared_hashes_norm <base> <out>
    local base="$1" out="$2"
    {
        sha256sum "$base/usr/lib/cachyos-kernel-manager/terminal-helper" | cut -d' ' -f1
        sha256sum "$base/usr/lib/cachyos-kernel-manager/rootshell.sh" | cut -d' ' -f1
        sed -e '/^# KWin groups all three windows/,/^StartupWMClass=org.cachyos.KernelManager$/d' \
            "$base/usr/share/applications/org.cachyos.KernelManager.desktop" \
            | sha256sum | cut -d' ' -f1
        sha256sum "$base/usr/share/polkit-1/actions/org.cachyos.KernelManager.pkexec.policy" | cut -d' ' -f1
        for d in 16x16 22x22 32x32 44x44 48x48 64x64 128x128 150x150 256x256 310x310; do
            sha256sum "$base/usr/share/icons/hicolor/$d/apps/org.cachyos.KernelManager.png" | cut -d' ' -f1
        done
    } > "$out"
}

rm -rf /tmp/pkg-oracle-x && mkdir -p /tmp/pkg-oracle-x
bsdtar -xf "$PKG" -C /tmp/pkg-oracle-x
extract_shared_hashes /tmp/pkg-oracle-x "$ORACLE/shared-files.sha256"
extract_shared_hashes_norm /tmp/pkg-oracle-x "$ORACLE/shared-files.normalized.sha256"

# the candidate's packaging tree has the icons WITHOUT the apps/ dir; the
# PKGBUILD installs them INTO apps/ — materialize the install layout
rm -rf /tmp/pkg-cand-x && mkdir -p /tmp/pkg-cand-x
cp -r "$ROOT/packaging/usr" /tmp/pkg-cand-x/
for d in 16x16 22x22 32x32 44x44 48x48 64x64 128x128 150x150 256x256 310x310; do
    mkdir -p "/tmp/pkg-cand-x/usr/share/icons/hicolor/$d/apps"
    mv "/tmp/pkg-cand-x/usr/share/icons/hicolor/$d/org.cachyos.KernelManager.png" \
        "/tmp/pkg-cand-x/usr/share/icons/hicolor/$d/apps/"
done
extract_shared_hashes /tmp/pkg-cand-x "$CANDIDATE/shared-files.sha256"
extract_shared_hashes_norm /tmp/pkg-cand-x "$CANDIDATE/shared-files.normalized.sha256"
rm -rf /tmp/pkg-oracle-x /tmp/pkg-cand-x

echo "packaging witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run packaging/file-layout"
