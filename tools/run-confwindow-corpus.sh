#!/usr/bin/env bash
#
# run-confwindow-corpus.sh — witness runner for the
# ui/configure-window-semantics court (Phase 8).
#
# For every frozen corpus file of the court, run the oracle reference CLI
# (the conf-window.cpp semantics re-declared from the frozen source) and the
# candidate CLI (the ui crate's configure_window model), and record:
#   oracle/<name>.json       the configure-window model JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# Then: cargo xtask court run ui/configure-window-semantics
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_BIN="$ROOT/target/release/confwindow-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-confwindow"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p confwindow-oracle-ref -p cachyos-kernel-manager-ui --bins

case_dir="$ROOT/courts/ui/configure-window-semantics"
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

echo "configure-window witness written to courts/ui/configure-window-semantics/{oracle,candidate}"
echo "compare: cargo xtask court run ui/configure-window-semantics"
