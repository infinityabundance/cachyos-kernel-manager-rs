#!/usr/bin/env bash
#
# stale-db — the cachyos sync database file is removed. The oracle still
# REGISTERS the section (mINI sees [cachyos]) but libalpm finds no packages
# for it: cachyos/linux-cachyos disappears from discovery while core/extra
# kernels remain.
#
set -euo pipefail
rm -f /var/lib/pacman/sync/cachyos.db
echo "fixture stale-db: cachyos.db removed"
