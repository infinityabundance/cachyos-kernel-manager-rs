#!/usr/bin/env bash
#
# kernel-install — Phase 11 boot/system court fixture: the base system with
# a REAL second kernel package CACHED (not installed) so the court's
# install mutation runs the real post-install hooks (mkinitcpio +
# bootloader) OFFLINE.
#
# The base image runs linux-cachyos 7.1.8-1 (the qemu direct-kernel boot).
# This fixture refreshes the sync dbs and caches linux-cachyos-lts +
# headers (from the CachyOS repos, downloaded at bake time — the baker has
# network; the court VMs do not). No fixtures repo is needed (the install
# command targets the cachyos repo).
#
set -euo pipefail

# refresh the sync dbs (the base's may be stale)
pacman -Sy --noconfirm >/tmp/sync.log 2>&1 || { cat /tmp/sync.log >&2; exit 1; }

# cache the real lts kernel + headers (the court installs them via the
# courted pacman command; the post-install hooks regenerate /boot)
pacman -Sw --noconfirm --cachedir /var/cache/pacman/pkg \
    linux-cachyos-lts linux-cachyos-lts-headers >/tmp/lts-download.log 2>&1 \
    || { cat /tmp/lts-download.log >&2; exit 1; }

echo "fixture kernel-install: base kernel running, lts cached (not installed)"
