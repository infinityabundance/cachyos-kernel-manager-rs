#!/usr/bin/env bash
#
# run-buildflow-corpus.sh — witness runner for the build-env/lifecycle court.
#
# For every frozen corpus file (variant + cwd + artifact globs), run the
# oracle reference CLI (on_execute/finished_proc/aur_kernel decisions,
# conf-window.cpp:696-735/378-405 + aur_kernel.cpp:53) and the candidate CLI
# (BuildFlowPlan), and record:
#   oracle/<name>.json          the plan JSON
#   oracle/<name>.exit          exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# stderr is dropped: error texts are implementation-specific.
# Then: cargo xtask court run build-env/lifecycle
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/build-env/lifecycle"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/buildflow-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-buildflow"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p buildflow-oracle-ref -p cachyos-kernel-manager-exec --bins

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

echo "buildflow corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run build-env/lifecycle"
