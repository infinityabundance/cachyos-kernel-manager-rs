#!/usr/bin/env bash
#
# fixture-lib.sh — helpers sourced by fixture bake scripts (run inside the
# chroot via arch-chroot). Builds fake packages into the fixtures repo and
# syncs it.
#
# The fixtures repo (/srv/cachyos-km-fixtures) is a FILE-based pacman repo.
# Because discovery registers every non-options/testing pacman.conf section
# as a sync database, the `[fixtures]` repo is itself observable by BOTH the
# oracle and the candidate — which is exactly what the custom-repo,
# duplicate-across-repos and upgrade/downgrade courts rely on.
#
set -euo pipefail

FIXTURES_REPO_DIR="${FIXTURES_REPO_DIR:-/srv/cachyos-km-fixtures}"
FIXTURES_REPO_NAME="fixtures"

ensure_fixtures_repo() {
    mkdir -p "$FIXTURES_REPO_DIR"
    chown -R test:test "$FIXTURES_REPO_DIR"
    if ! grep -q "^\[$FIXTURES_REPO_NAME\]" /etc/pacman.conf; then
        cat >> /etc/pacman.conf <<EOF

[$FIXTURES_REPO_NAME]
Server = file://$FIXTURES_REPO_DIR
SigLevel = Never
EOF
    fi
}

# build_fakepkg <name> <pkgver> <pkgrel> [epoch]
build_fakepkg() {
    local name="$1" ver="$2" rel="$3" epoch="${4:-0}"
    # the repo dir must exist BEFORE the artifact install below (the dir is
    # otherwise only created by repo_add_all -> ensure_fixtures_repo, and
    # install to a missing dir with a trailing slash fails "Not a directory").
    # NOTE: only the DIRECTORY is ensured here — the pacman.conf [fixtures]
    # section is added by ensure_fixtures_repo (repo_add_all) so fixtures
    # with a custom section name can control the conf themselves.
    mkdir -p "$FIXTURES_REPO_DIR"
    chown -R test:test "$FIXTURES_REPO_DIR"
    local work="/tmp/fakepkg-$name"
    rm -rf "$work"
    mkdir -p "$work"
    cat > "$work/PKGBUILD" <<EOF
pkgname=$name
pkgver=$ver
pkgrel=$rel
epoch=$epoch
pkgdesc="cachyos-km court fixture (fake package)"
arch=('any')
url="https://example.invalid"
license=('MIT')
package() {
    install -d "\$pkgdir/usr/share/cachyos-km-fixtures"
    echo "$name $ver-$rel" > "\$pkgdir/usr/share/cachyos-km-fixtures/$name"
}
EOF
    # makepkg refuses to run as root; build as the `test` user
    chown -R test:test "$work"
    (cd "$work" && sudo -u test makepkg -f --noconfirm --skipinteg)
    # copy the artifact to the repo dir, and keep a PRIVATE copy under a
    # deterministic `fakepkg-` prefix in /tmp for -U installs of OLDER
    # versions than the synced one (the specs reference these paths)
    local artifact
    artifact="$(ls "$work"/*.pkg.tar.zst 2>/dev/null | head -1)"
    [ -n "$artifact" ] || { echo "build_fakepkg: no artifact for $name" >&2; exit 1; }
    install -o test -g test -m 644 "$artifact" "$FIXTURES_REPO_DIR/"
    cp "$artifact" "/tmp/fakepkg-$(basename "$artifact")"
}

repo_add_all() {
    ensure_fixtures_repo
    # the db must be named after the REPO (`$repo.db` is what pacman fetches
    # from the Server URL) — repo-add's filename argument is arbitrary
    (cd "$FIXTURES_REPO_DIR" && repo-add -q -R "$FIXTURES_REPO_NAME.db.tar.zst" *.pkg.tar.zst)
}

pacman_sync() {
    pacman -Sy --noconfirm >/dev/null
}

# install a fake package from /tmp by file (bypasses the repo: the local
# provenance is then "unknown"/external, which courts the
# installed-from-other-repo semantics)
install_pkg_file() {
    local file="$1"
    pacman -U --noconfirm "$file" >/dev/null
}

# ---------------------------------------------------------------------------
# Phase 5 transaction-court fixtures: controlled probe wrappers + terminal.
# ---------------------------------------------------------------------------

# install_chwd_wrapper <profile...> — a /usr/local/bin/chwd that shadows the
# real chwd for the oracle's exact probe invocation and prints the REAL chwd
# output format (observed on a CachyOS host): a box table whose Name lines
# have the profile name in whitespace-field 4 — `awk '{print $4}'` picks it
# up (kernel.cpp:45). All other invocations delegate to the real chwd.
install_chwd_wrapper() {
    mkdir -p /usr/local/bin
    local script="/usr/local/bin/chwd"
    {
        printf '%s\n' '#!/usr/bin/bash'
        printf '%s\n' 'if [ "$1" = "--list-installed" ] && [ "$2" = "-d" ]; then'
        printf '%s\n' '    cat <<TABLE'
        printf '%s\n' '╭───────────┬───────────────────────────────────────────────╮'
        for p in "$@"; do
            printf '%s\n' "│ Name      ┆ ${p}"
            printf '%s\n' '│ Desc      ┆ court fixture profile'
            printf '%s\n' '├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤'
        done
        printf '%s\n' '╰───────────┴───────────────────────────────────────────────╯'
        printf '%s\n' 'TABLE'
        printf '%s\n' '    exit 0'
        printf '%s\n' 'fi'
        printf '%s\n' 'exec /usr/bin/chwd "$@"'
    } > "$script"
    chmod +x "$script"
}

# install_findmnt_wrapper <fstype> — a /usr/local/bin/findmnt that answers
# the oracle's exact probe (`findmnt -ln -o FSTYPE /`, kernel.cpp:41) with a
# controlled fstype and delegates everything else to the real findmnt.
install_findmnt_wrapper() {
    local fstype="$1"
    mkdir -p /usr/local/bin
    local script="/usr/local/bin/findmnt"
    {
        printf '%s\n' '#!/usr/bin/bash'
        printf '%s\n' 'if [ "$1" = "-ln" ] && [ "$2" = "-o" ] && [ "$3" = "FSTYPE" ] && [ "$4" = "/" ]; then'
        printf '%s\n' "    echo \"${fstype}\""
        printf '%s\n' '    exit 0'
        printf '%s\n' 'fi'
        printf '%s\n' 'exec /usr/bin/findmnt "$@"'
    } > "$script"
    chmod +x "$script"
}

# install_xterm — the transaction courts need a REAL terminal emulator so
# the oracle's terminal-helper chain (xterm -e bash <file>) actually reaches
# the pacman execve (the strace witness). xterm is the lightest in the
# helper's term_order.
install_xterm() {
    pacman -S --noconfirm --needed xterm >/dev/null
}

# install_terminal_stubs — stub emulator binaries used by the terminal-matrix
# court. Each stub records its argv to a log and exits with a configurable
# status. The stubs live under /usr/local/bin/stubs; the in-VM runner builds
# the PATH per scenario (the terminal-helper picks the first term_order entry
# that `command -v` finds).
install_terminal_stubs() {
    mkdir -p /usr/local/bin/stubs
    for t in alacritty kitty ptyxis konsole kgx gnome-terminal xfce4-terminal lxterminal xterm st foot rio ghostty; do
        cat > "/usr/local/bin/stubs/$t" <<EOF
#!/usr/bin/bash
STATUS=\${TERMINAL_STUB_STATUS:-0}
echo "stub $t \$*" >> /tmp/terminal-stub.log
exit "\$STATUS"
EOF
        chmod +x "/usr/local/bin/stubs/$t"
    done
}
