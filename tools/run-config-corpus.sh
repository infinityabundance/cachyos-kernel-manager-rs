#!/usr/bin/env bash
#
# run-config-corpus.sh — witness runner for the config-roundtrip/
# canonicalization court (directive §44, §21).
#
# For every frozen corpus file, run the oracle reference CLI (verbatim
# upstream struct + toml 1.1, the oracle's actual dependency) and the
# candidate CLI (KernelManagerConfig + toml 0.8), and record:
#   oracle/<name>.canonical     stdout (the canonical re-serialization)
#   oracle/<name>.exit          exit code (0 parse ok / 1 parse error)
#   candidate/<name>.canonical
#   candidate/<name>.exit
#
# stderr is deliberately dropped: the parse-error texts differ between the
# C++ fmt surface and Rust and are outside the behavioral contract.
#
# Then: cargo xtask court run config-roundtrip/canonicalization
# fingerprints oracle/ vs candidate/ and reports PASS/FAIL.
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/config-roundtrip/canonicalization"
CORPUS="$CASE/corpus"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/config-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-config"

# Build both witnesses (release so the runner matches the court evidence).
cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p config-oracle-ref -p cachyos-kernel-manager-config --bins

mkdir -p "$ORACLE" "$CANDIDATE"

run_one() { # run_one <bin> <outdir> <file>
    local bin="$1" outdir="$2" file="$3"
    local name; name="$(basename "$file" .toml)"
    # capture stdout only; stderr is not part of the contract. The `if`
    # form preserves the exit code without tripping `set -e`.
    local stdout rc
    if stdout="$("$bin" parse "$file" 2>/dev/null)"; then
        rc=0
    else
        rc=$?
    fi
    printf '%s' "$stdout" > "$outdir/$name.canonical"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for f in "$CORPUS"/*.toml; do
    run_one "$ORACLE_BIN" "$ORACLE" "$f"
    run_one "$CANDIDATE_BIN" "$CANDIDATE" "$f"
done

echo "corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run config-roundtrip/canonicalization"
