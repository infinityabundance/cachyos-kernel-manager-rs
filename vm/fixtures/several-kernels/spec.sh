#!/usr/bin/env bash
#
# several-kernels — several REAL kernels installed from multiple repos:
#   core:    linux, linux-lts
#   extra:   linux-zen
#   cachyos: linux-cachyos (base), linux-cachyos-lts, linux-cachyos-rt-bore
#
# Exercises multiple categories (stable/longterm/zen-kernel/release-candidate
# via -rc not installed here), cross-repo rows, and installed companions.
#
set -euo pipefail
pacman -S --noconfirm --needed \
    linux linux-headers \
    linux-lts linux-lts-headers \
    linux-zen linux-zen-headers \
    linux-cachyos-lts linux-cachyos-lts-headers \
    linux-cachyos-rt-bore linux-cachyos-rt-bore-headers \
    >/dev/null
echo "fixture several-kernels: installed"
