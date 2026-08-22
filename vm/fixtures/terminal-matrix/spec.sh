#!/usr/bin/env bash
#
# terminal-matrix — stub terminal emulators for the terminal-helper matrix
# court (gap-005/gap-008). The stubs live in /usr/local/bin/stubs; the
# in-VM runner builds per-scenario PATHs (the helper picks the first
# term_order entry that `command -v` finds). Each stub logs its argv and
# exits with TERMINAL_STUB_STATUS (default 0).
#
# The scenarios court the script's exit-code surface:
#   none (no emulator found)  -> notify-send + exit 1
#   first-pick fails          -> exit 2 (file removed)
#   kgx fails                 -> exit 0 (the kgx special case)
#   emulator succeeds         -> exit 0
#   -s <shell> override       -> LAUNCHER_CMD change
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

install_terminal_stubs
pacman -S --noconfirm --needed libnotify >/dev/null
echo "fixture terminal-matrix: emulator stubs installed"
