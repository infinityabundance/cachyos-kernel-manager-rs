#!/usr/bin/env bash
#
# malformed — a deliberately malformed pacman.conf exercising mINI's exact
# line semantics:
#   `[a=b`        '[' line WITHOUT ']' -> falls through to key/value: key
#                 `[a` in the auto section "0" (registered as a repo!)
#   `[broken`     no ']' and no '=' -> PDATA_UNKNOWN, ignored
#   `garbage`     no '=' -> PDATA_UNKNOWN, ignored
#   `[fixtures]`  normal section -> registered
# So the oracle registers "0" and "fixtures". (The "0" repo has no db file
# -> registered but empty.) The db is placed directly into sync/ (no
# pacman -Sy on a broken config).
#
set -euo pipefail
source /opt/cachyos-km-vm/fixture-lib.sh

build_fakepkg linux-cachyos-mal 9.9.9 1
build_fakepkg linux-cachyos-mal-headers 9.9.9 1
repo_add_all

# replace the pacman.conf with the malformed variant (keep the base repos'
# sections so the base kernels stay discoverable)
cat > /etc/pacman.conf <<EOF
[a=b
[broken
garbage
[options]
Architecture = auto

[cachyos]
Server = https://mirror.cachyos.org/repo/x86_64/cachyos

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist

[fixtures]
Server = file://$FIXTURES_REPO_DIR
SigLevel = Never
EOF
cp "$FIXTURES_REPO_DIR/fixtures.db" /var/lib/pacman/sync/fixtures.db
echo "fixture malformed: [a=b key, [broken ignored, auto section '0'"
