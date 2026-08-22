#!/usr/bin/env bash
#
# run-aur-corpus.sh — witness runner for the aur/enablement-matrix court.
#
# For every frozen corpus file (feature flag, paru/awk availability, repo
# kernel names, paru probe output, AUR selections, pre-expanded repo
# install/remove lists), run the oracle reference CLI (kernel.cpp:253-283
# discovery + 288-304 commit + aur_kernel.cpp:32-55, source-derived) and the
# candidate CLI (the real AUR model in the plan crate), and record:
#   oracle/<name>.json       the model JSON
#   oracle/<name>.stderr     stderr (the gate message is an OBSERVABLE —
#                            byte-compared, NOT ignored)
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.stderr
#   candidate/<name>.exit
#
# Then: cargo xtask court run aur/enablement-matrix
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/aur/enablement-matrix"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/aur-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-aur-model"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p aur-oracle-ref -p cachyos-kernel-manager-plan --bins

mkdir -p "$ORACLE" "$CANDIDATE"

run_one() { # run_one <bin> <outdir> <file>
    local bin="$1" outdir="$2" file="$3"
    local name; name="$(basename "$file" .json)"
    local stdout stderr rc
    set +e
    stdout="$("$bin" parse "$file" 2>"$outdir/$name.stderr.tmp")"
    rc=$?
    set -e
    printf '%s' "$stdout" > "$outdir/$name.json"
    mv "$outdir/$name.stderr.tmp" "$outdir/$name.stderr"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for f in "$CORPUS"/*.json; do
    run_one "$ORACLE_BIN" "$ORACLE" "$f"
    run_one "$CANDIDATE_BIN" "$CANDIDATE" "$f"
done

echo "aur corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run aur/enablement-matrix"
