#!/usr/bin/env bash
#
# run-option-corpus.sh — witness runner for the option-transitions/
# variant-switch court.
#
# For every frozen switch-sequence file, run the oracle reference CLI (the
# variant-switch handler conf-window.cpp:553-602) and the candidate CLI
# (VariantSwitchState model), and record:
#   oracle/<name>.json          the state after each switch
#   oracle/<name>.exit          exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# stderr is dropped: error texts are implementation-specific.
# Then: cargo xtask court run option-transitions/variant-switch
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/option-transitions/variant-switch"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/options-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-variant-switch"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p options-oracle-ref -p cachyos-kernel-manager-build --bins

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

echo "option corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run option-transitions/variant-switch"
