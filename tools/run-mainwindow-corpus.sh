#!/usr/bin/env bash
#
# run-mainwindow-corpus.sh — witness runner for the
# ui/main-window-semantics court (Phase 8).
#
# For every frozen corpus file of the court, run the oracle reference CLI
# (the km-window.cpp semantics re-declared from the frozen source) and the
# candidate CLI (the ui crate's main_window model), and record:
#   oracle/<name>.json       the main-window model JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run ui/main-window-semantics
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_BIN="$ROOT/target/release/mainwindow-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-mainwindow"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p mainwindow-oracle-ref -p cachyos-kernel-manager-ui --bins

case_dir="$ROOT/courts/ui/main-window-semantics"
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

echo "main-window witness written to courts/ui/main-window-semantics/{oracle,candidate}"
echo "compare: cargo xtask court run ui/main-window-semantics"
