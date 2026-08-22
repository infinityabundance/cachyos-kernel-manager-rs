#!/usr/bin/env bash
#
# missing-conf — /etc/pacman.conf does not exist. mINI cannot open the file,
# so the structure is EMPTY and the oracle registers ZERO sync databases:
# discovery is empty and the "No kernels found!" dialog is shown. The
# candidate must produce the same empty registration (it tolerates the
# missing file instead of panicking).
#
set -euo pipefail
rm -f /etc/pacman.conf
echo "fixture missing-conf: no /etc/pacman.conf, zero registrations"
