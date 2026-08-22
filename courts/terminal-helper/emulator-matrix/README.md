# terminal-helper/emulator-matrix

Courts the `terminal-helper` exit-code surface per emulator (gap-005/gap-008)
using stub emulators whose exit status is controlled by the fixture (the
narrowest verifiable simulation boundary: the emulator binary is an external
authority, the script's decision logic is what we prove).

Scenarios:

| scenario | stub | expected exit |
|---|---|---|
| none | no emulator in PATH | 1 (notify-send + exit 1) |
| first-fails | alacritty exits 1 | 2 (non-kgx failure path, file removed) |
| kgx-fails | kgx exits 1 | 0 (kgx special case) |
| success | xterm exits 0 | 0 |
| shell-option | -s echo | 0 (launcher change) |

Both sides run the same script: the oracle the frozen upstream copy, the
candidate the packaged copy (`packaging/usr/lib/cachyos-kernel-manager/`).
The comparator compares exit codes + outputs after temp-path normalization.

Run: `cargo xtask court run terminal-helper/emulator-matrix --vm`
