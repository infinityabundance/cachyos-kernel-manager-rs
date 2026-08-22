#!/usr/bin/env bash
#
# run-i18n-corpus.sh — witness runner for the ui/i18n-resolution court
# (Phase 8).
#
# For every frozen corpus file, run the oracle reference CLI (parses the
# frozen lang/*.ts XML directly) and the candidate CLI (the ui crate's
# embedded catalogs + resolution), and record:
#   oracle/<name>.json       the resolved-translation JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run ui/i18n-resolution
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_BIN="$ROOT/target/release/i18n-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-i18n"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p i18n-oracle-ref -p cachyos-kernel-manager-ui --features rendering --bins

case_dir="$ROOT/courts/ui/i18n-resolution"
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

echo "i18n witness written to courts/ui/i18n-resolution/{oracle,candidate}"
echo "compare: cargo xtask court run ui/i18n-resolution"
