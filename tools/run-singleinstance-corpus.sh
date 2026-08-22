#!/usr/bin/env bash
#
# run-singleinstance-corpus.sh — witness runner for the
# single-instance/stale-lock court (Phase 9).
#
# For every frozen corpus file, run the oracle reference CLI (the
# IsInstanceAlreadyRunning decision re-declared from the frozen source) and
# the candidate CLI (the platform crate's single_instance model), and
# record:
#   oracle/<name>.json       the decision JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run single-instance/stale-lock
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_BIN="$ROOT/target/release/singleinstance-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-single-instance"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p singleinstance-oracle-ref -p cachyos-kernel-manager-platform --bins

case_dir="$ROOT/courts/single-instance/stale-lock"
mkdir -p "$case_dir/oracle" "$case_dir/candidate"

for f in "$case_dir"/fixture/corpus/*.json; do
    name="$(basename "$f" .json)"
    stdout="$("$ORACLE_BIN" parse "$f" 2>/dev/null)" || rc=$?
    printf '%s' "$stdout" > "$case_dir/oracle/$name.json"
    printf '%d' "${rc:-0}" > "$case_dir/oracle/$name.exit"
    rc=0
    stdout="$("$CANDIDATE_BIN" parse "$f" 2>/dev/null)" || rc=$?
    printf '%s' "$stdout" > "$case_dir/candidate/$name.json"
    printf '%d' "${rc:-0}" > "$case_dir/candidate/$name.exit"
    rc=0
done

echo "single-instance witness written to courts/single-instance/stale-lock/{oracle,candidate}"
echo "compare: cargo xtask court run single-instance/stale-lock"
