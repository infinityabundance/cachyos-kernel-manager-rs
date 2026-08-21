#!/usr/bin/env bash
#
# residual.sh — capture a machine residual (directive §42).
#
# Deterministic, hashable description of the package state the oracle and
# candidate observe. Used both as court evidence and as the fixture digest
# input. Timestamps/PIDs are deliberately excluded (they are normalizers'
# job to drop, not ours to embed).
#
# NOTE: deliberately NOT `set -e`: a failing probe (e.g. unreadable /boot)
# must never truncate the JSON — the residual is evidence and must always
# be well-formed, degrading individual fields instead.
set -uo pipefail

echo '{'
echo '  "schema": "cachyos-km-machine-residual-v1",'
echo -n '  "machine_id": "'
if [ -f /etc/machine-id ]; then tr -d '\n' < /etc/machine-id; fi
echo '",'
echo -n '  "fixture_marker_present": '
if [ -f /etc/cachyos-km/fixture.marker ]; then echo 'true,'; else echo 'false,'; fi
echo -n '  "kernel": "'; uname -r | tr -d '\n'; echo '",'
echo -n '  "os_release_id": "'; . /etc/os-release; echo -n "$ID ${VERSION_ID:-unknown}"; echo '",'

echo '  "installed_packages": ['
pacman -Q | sort | sed 's/\\/\\\\/g; s/"/\\"/g; s/^/    "/; s/$/",/' | sed '$ s/,$//'
echo '  ],'

echo '  "sync_db_hashes": {'
for db in /var/lib/pacman/sync/*.db; do
    [ -e "$db" ] || continue
    echo -n "    \"$(basename "$db")\": \"$(sha256sum "$db" | awk '{print $1}')\","
    echo
done | sed '$ s/,$//'
echo '  },'

echo '  "local_db_packages": ['
for desc in /var/lib/pacman/local/*/desc; do
    [ -e "$desc" ] || continue
    name=$(awk '/^%NAME%/{getline; print}' "$desc")
    ver=$(awk '/^%VERSION%/{getline; print}' "$desc")
    # escape backslashes and double quotes so package names cannot break the JSON
    printf '    "%s %s",\n' "$(printf '%s' "$name" | sed 's/\\/\\\\/g; s/"/\\"/g')" "$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g')"
done | sort | sed '$ s/,$//'
echo '  ],'

echo -n '  "pacman_conf_sha256": "'; sha256sum /etc/pacman.conf | awk '{print $1}' | tr -d '\n'; echo '",'
echo -n '  "boot_tree": "'; (cd /boot && find . -type f 2>/dev/null | sort | xargs -r sha256sum 2>/dev/null | sha256sum | awk '{print $1}' | tr -d '\n'); echo '"'
echo '}'
