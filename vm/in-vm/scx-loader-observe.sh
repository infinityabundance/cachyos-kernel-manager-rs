#!/usr/bin/env bash
#
# scx-loader-observe.sh — the ORACLE side of the scx/loader-interface VM
# court: start the REAL scx_loader on the system bus (the reference image
# ships the scx-manager package), introspect org.scx.Loader, and read the
# loader's property values — the strongest witness for the D-Bus surface
# and the state readback.
#
set -euo pipefail

OUT="${1:-/mnt/host/out}"
mkdir -p "$OUT"

if [ -f /mnt/host/scripts/scx-loader-observe.sh ] && [ "$0" != "/mnt/host/scripts/scx-loader-observe.sh" ]; then
    exec /mnt/host/scripts/scx-loader-observe.sh "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi

if ! command -v scx_loader >/dev/null 2>&1; then
    echo "FATAL: scx_loader not installed in the reference image" >&2
    exit 4
fi

# Start the loader (idempotent: already-active is fine). It may need a
# moment to register the name.
systemctl start scx_loader 2>/dev/null || true
for i in $(seq 1 10); do
    if busctl list 2>/dev/null | grep -q 'org.scx.Loader'; then
        break
    fi
    sleep 1
done

busctl introspect org.scx.Loader /org/scx/Loader > "$OUT/introspect.txt" 2>"$OUT/oracle.stderr" || {
    echo "FATAL: busctl introspect failed (loader not on the bus)" >&2
    exit 5
}

# Every property value (the state readback). The frozen oracle reads
# CurrentScheduler / SchedulerMode / SupportedSchedulers; the newer loader
# may expose more — record them ALL.
{
    echo "{"
    first=1
    while read -r name sig value; do
        [ "$name" = "NAME" ] && continue
        case "$name" in
            *.CurrentScheduler|*.SchedulerMode|*.SupportedSchedulers|*.CurrentSchedulerArgs|*.DefaultMode|*.DefaultScheduler|*.Running)
                [ "$first" -eq 0 ] && echo ","
                first=0
                prop="${name##*.}"
                if [ "$sig" = "as" ]; then
                    # busctl's column layout may truncate long arrays; use
                    # get-property for the authoritative list and convert
                    # `as <n> "a" "b" ...` into a JSON array.
                    raw="$(busctl get-property org.scx.Loader /org/scx/Loader org.scx.Loader "$prop" 2>/dev/null | sed 's/^as[[:space:]]*//')"
                    json="["
                    first_item=1
                    set -f  # no globbing on the array elements
                    read -ra items <<< "$raw"
                    for item in "${items[@]:1}"; do  # skip the count
                        [ "$first_item" -eq 0 ] && json="$json, "
                        first_item=0
                        json="$json$item"
                    done
                    set +f
                    json="$json]"
                    printf '"%s": %s' "$prop" "$json"
                else
                    printf '"%s": %s' "$prop" "$value"
                fi
                ;;
        esac
    done < <(busctl introspect org.scx.Loader /org/scx/Loader | awk '$1 ~ /^\./ && $2 == "property" {print $1, $3, $4}')
    echo ""
    echo "}"
} > "$OUT/oracle-properties.json"

# the loader version (provenance of the superset)
pacman -Q scx-manager 2>/dev/null > "$OUT/scx-manager-version.txt" || true

echo "scx loader observation complete"
