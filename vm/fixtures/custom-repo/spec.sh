#!/usr/bin/env bash
#
# custom-repo — the `[fixtures]` file-based repo carries a fake kernel NOT
# installed anywhere: discovery must produce the row "fixtures/linux-cachyos-
# court" (unchecked, not installed).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-court 9.9.9 1
build_fakepkg linux-cachyos-court-headers 9.9.9 1
repo_add_all
pacman_sync
echo "fixture custom-repo: fixtures/linux-cachyos-court available"
