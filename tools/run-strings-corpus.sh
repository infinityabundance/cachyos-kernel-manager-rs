#!/usr/bin/env bash
#
# run-strings-corpus.sh — witness runner for the ui/dialog-strings court.
#
# The string table is fixed (no corpus): the oracle reference CLI
# re-declares the strings from the frozen source; the candidate CLI renders
# the strings module. Both write:
#   oracle/strings.json       the strings JSON
#   oracle/strings.exit       exit code
#   candidate/strings.json
#   candidate/strings.exit
#
# Then: cargo xtask court run ui/dialog-strings
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/ui/dialog-strings"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/strings-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-strings"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p strings-oracle-ref -p cachyos-kernel-manager-ui --bins

mkdir -p "$ORACLE" "$CANDIDATE"

"$ORACLE_BIN" > "$ORACLE/strings.json" 2>/dev/null
printf '%d' "$?" > "$ORACLE/strings.exit"
"$CANDIDATE_BIN" > "$CANDIDATE/strings.json" 2>/dev/null
printf '%d' "$?" > "$CANDIDATE/strings.exit"

echo "strings witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run ui/dialog-strings"
