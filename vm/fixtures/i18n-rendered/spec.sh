#!/usr/bin/env bash
#
# i18n-rendered — Phase 12 hostile-review fixture: the RENDERED i18n court.
#
# Extends the gui-integration fixture (the winit X11 client stack +
# at-spi2-core-2.52.0 + a font) with GENERATED non-English locales: the
# court drives the packaged GUIs (oracle Qt / candidate Slint) under
# LANG=de_DE.UTF-8 and LANG=zh-CN.UTF-8 and compares the RENDERED main
# window's accessible projection (description + tree headers + buttons).
#
# The ORACLE (Qt) resolves QLocale::system() from the ACTUAL locale — an
# ungenerated de_DE.UTF-8 silently falls back to C and never loads the
# embedded de catalog — so the locale MUST exist in the machine. The
# candidate parses the LANG/LC_ALL env string directly (no OS locale
# needed), but the same-machine witness requires both sides to resolve the
# SAME way.
#
# zh_CN.UTF-8 is generated for the gap-009 rendered projection: Qt's
# QLocale reports zh_CN (underscore) while the frozen qrc alias is zh-CN
# (dash) — the oracle NEVER loads its CJK catalog; the candidate
# reproduces that miss (ui/i18n-resolution). The court witnesses both
# sides render the ENGLISH projection for zh_CN.
#
set -euo pipefail

# the gui-integration prerequisites (winit X11 client stack, at-spi2-core
# pinned for accesskit_unix 0.22.1, a font)
cat > /tmp/gui-spec.sh <<'SPEC'
#!/usr/bin/env bash
set -euo pipefail
pacman -Sy --noconfirm >/tmp/sync.log 2>&1 || { cat /tmp/sync.log >&2; exit 1; }
curl -fsSL -o /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    https://archive.archlinux.org/packages/a/at-spi2-core/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-dl.log 2>&1 || { cat /tmp/atspi-dl.log >&2; exit 1; }
pacman -U --noconfirm /tmp/at-spi2-core-2.52.0-1-x86_64.pkg.tar.zst \
    >/tmp/atspi-downgrade.log 2>&1 || { cat /tmp/atspi-downgrade.log >&2; exit 1; }
pacman -Q at-spi2-core
pacman -S --noconfirm --needed \
    libx11 libxcb libxkbcommon libxkbcommon-x11 \
    libxcursor libxi libxrandr libxinerama libxft \
    xorg-xrandr xdotool ttf-dejavu \
    >/tmp/gui-install.log 2>&1 || { cat /tmp/gui-install.log >&2; exit 1; }
SPEC
bash /tmp/gui-spec.sh

# the GENERATED locales (the machine-level resolution both GUIs use)
localedef -i de_DE -f UTF-8 de_DE.UTF-8 >/tmp/localedef-de.log 2>&1 \
    || { cat /tmp/localedef-de.log >&2; exit 1; }
localedef -i zh_CN -f UTF-8 zh_CN.UTF-8 >/tmp/localedef-zh.log 2>&1 \
    || { cat /tmp/localedef-zh.log >&2; exit 1; }
locale -a | grep -E "de_DE|zh_CN" || { echo "locales not generated" >&2; exit 1; }

echo "fixture i18n-rendered: de_DE.UTF-8 + zh_CN.UTF-8 ready (at-spi2-core $(pacman -Qq at-spi2-core))"
