#!/usr/bin/env bash
#
# terminal-matrix-run.sh — court the terminal-helper exit-code surface per
# emulator (gap-005, gap-008) against a given helper script.
#
# Usage: terminal-matrix-run.sh <helper-path> <out-dir>
#
# Produces <out-dir>/terminal-matrix.json (schema
# cachyos-km-terminal-matrix-v1): one record per scenario with the raw
# exit code, stdout, stderr and the temp-file-removal residual.
#
# Scenarios (the emulator is an external authority; the stub controls its
# exit status deterministically — the narrowest verifiable simulation
# boundary for the script's decision logic):
#   none          no emulator in PATH            -> notify-send + exit 1
#   first-fails   alacritty stub exits 1         -> exit 2 (+ rm file)
#   kgx-fails     kgx stub exits 1               -> exit 0 (kgx special case)
#   success       xterm stub exits 0             -> exit 0
#   shell-option  -s echo override               -> LAUNCHER_CMD change
#
set -euo pipefail

HELPER="$1"
OUT="${2:-/mnt/host/out}"
mkdir -p "$OUT"

# --- share re-exec: prefer the current revision ---
if [ -f /mnt/host/scripts/terminal-matrix-run.sh ] && [ "$0" != "/mnt/host/scripts/terminal-matrix-run.sh" ]; then
    exec /mnt/host/scripts/terminal-matrix-run.sh "$HELPER" "$OUT"
fi

if [ ! -f /etc/cachyos-km/fixture.marker ]; then
    echo "REFUSING: not an approved court VM" >&2
    exit 3
fi
[ -x "$HELPER" ] || { echo "FATAL: helper not executable: $HELPER" >&2; exit 4; }

STUBS=/usr/local/bin/stubs
BASE_PATH="/usr/bin:/bin"

mk_scen() {
    local name="$1" target="$2"
    local dir="/tmp/tm-$name"
    rm -rf "$dir"
    mkdir -p "$dir"
    if [ -n "$target" ]; then
        ln -s "$STUBS/$target" "$dir/$target"
    fi
    echo "$dir"
}

run_scen() {
    local name="$1" status="$2" dir="$3"
    shift 3
    local before after
    before="$(ls /tmp/tmp.* 2>/dev/null | wc -l)"
    set +e
    local stdout stderr rc
    stdout="$(env PATH="$dir:$BASE_PATH" TERMINAL_STUB_STATUS="$status" "$HELPER" "$@" 2>/tmp/tm-stderr)"; rc=$?
    set -e
    stderr="$(cat /tmp/tm-stderr 2>/dev/null || true)"
    after="$(ls /tmp/tmp.* 2>/dev/null | wc -l)"
    python3 - "$name" "$rc" "$stdout" "$stderr" "$((after - before))" <<'PY'
import json, sys
name, rc, stdout, stderr, leftover = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5])
print(json.dumps({"scenario": name, "exit": rc, "stdout": stdout, "stderr": stderr, "tmp_leftover": leftover}))
PY
}

SCEN_NONE="$(mk_scen none "")"
SCEN_FIRST="$(mk_scen first alacritty)"
SCEN_KGX="$(mk_scen kgx kgx)"
SCEN_OK="$(mk_scen ok xterm)"

RESULTS=()
RESULTS+=("$(run_scen none 0 "$SCEN_NONE" "echo hello")")
RESULTS+=("$(run_scen first-fails 1 "$SCEN_FIRST" "echo hello")")
RESULTS+=("$(run_scen kgx-fails 1 "$SCEN_KGX" "echo hello")")
RESULTS+=("$(run_scen success 0 "$SCEN_OK" "echo hello")")
RESULTS+=("$(run_scen shell-option 0 "$SCEN_OK" -s echo "echo hello")")

python3 - "$OUT/terminal-matrix.json" "${RESULTS[@]}" <<'PY'
import json, sys
out = sys.argv[1]
records = [json.loads(r) for r in sys.argv[2:]]
payload = {"schema": "cachyos-km-terminal-matrix-v1", "scenarios": records}
with open(out, "w") as f:
    json.dump(payload, f, indent=1)
print(f"terminal matrix: {len(records)} scenarios recorded")
PY
