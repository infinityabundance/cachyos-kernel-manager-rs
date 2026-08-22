#!/usr/bin/env bash
#
# run-cancel-corpus.sh — witness runner for the build-env/cancellation
# court.
#
# For every frozen corpus file (a sequence of Configure-window actions),
# run the oracle reference CLI (on_execute guard + closeEvent,
# conf-window.cpp:688-701, source-derived) and the candidate CLI (the exec
# crate's configure_trace model), and record:
#   oracle/<name>.json       the trace JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run build-env/cancellation
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/build-env/cancellation"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/cancel-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-cancel"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p cancel-oracle-ref -p cachyos-kernel-manager-exec --bins

mkdir -p "$ORACLE" "$CANDIDATE"

run_one() { # run_one <bin> <outdir> <file>
    local bin="$1" outdir="$2" file="$3"
    local name; name="$(basename "$file" .json)"
    local stdout rc
    if stdout="$("$bin" parse "$file" 2>/dev/null)"; then
        rc=0
    else
        rc=$?
    fi
    printf '%s' "$stdout" > "$outdir/$name.json"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for f in "$CORPUS"/*.json; do
    run_one "$ORACLE_BIN" "$ORACLE" "$f"
    run_one "$CANDIDATE_BIN" "$CANDIDATE" "$f"
done

echo "cancel corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run build-env/cancellation"
