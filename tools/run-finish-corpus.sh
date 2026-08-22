#!/usr/bin/env bash
#
# run-finish-corpus.sh — witness runner for the build-env/failure-lifecycle
# court.
#
# For every frozen corpus file (a sequence of async-process completions),
# run the oracle reference CLI (finished_proc, conf-window.cpp:378-405,
# source-derived) and the candidate CLI (the exec crate's finished_proc
# model), and record:
#   oracle/<name>.json       the outcome JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run build-env/failure-lifecycle
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/build-env/failure-lifecycle"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/finish-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-finish"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p finish-oracle-ref -p cachyos-kernel-manager-exec --bins

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

echo "finish corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run build-env/failure-lifecycle"
