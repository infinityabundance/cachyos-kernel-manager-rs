#!/usr/bin/env bash
#
# run-env-corpus.sh — witness runner for the build-env/env-rendering court.
#
# For every frozen corpus file (a UI option state), run the oracle reference
# CLI (get_all_set_values byte-for-byte: conf-window.cpp:421-451 +
# compile_options.json option_map) and the candidate CLI
# (BuildOptions::env_string), and record:
#   oracle/<name>.env          stdout (the env string)
#   oracle/<name>.exit         exit code (0 render ok / 1 invalid state)
#   candidate/<name>.env
#   candidate/<name>.exit
#
# stderr is dropped: error texts are implementation-specific.
# Then: cargo xtask court run build-env/env-rendering
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/build-env/env-rendering"
CORPUS="$CASE/fixture/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/env-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-env"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p env-oracle-ref -p cachyos-kernel-manager-build --bins

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
    printf '%s' "$stdout" > "$outdir/$name.env"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for f in "$CORPUS"/*.json; do
    run_one "$ORACLE_BIN" "$ORACLE" "$f"
    run_one "$CANDIDATE_BIN" "$CANDIDATE" "$f"
done

echo "env corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run build-env/env-rendering"
