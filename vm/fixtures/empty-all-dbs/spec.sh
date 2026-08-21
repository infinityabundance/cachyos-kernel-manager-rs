#!/usr/bin/env bash
#
# empty-all-dbs — every sync database removed: discovery finds NO kernels;
# the oracle must show the "No kernels found!\nPlease run `pacman -Sy`..."
# critical dialog. This is the negative-space court for discovery.
#
set -euo pipefail
rm -f /var/lib/pacman/sync/*.db
echo "fixture empty-all-dbs: no sync databases"
