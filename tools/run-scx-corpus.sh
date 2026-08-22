#!/usr/bin/env bash
#
# run-scx-corpus.sh — witness runner for the scx/* courts (Phase 7).
#
# For every frozen corpus file of every scx court surface, run the oracle
# reference CLI (the pre-extraction scx-manager at f3eeaf6 + scx_loader
# 1.0.9 decisions, source-derived) and the candidate CLI (the scx crate's
# models), and record:
#   oracle/<name>.json       the surface JSON
#   oracle/<name>.exit       exit code
#   candidate/<name>.json
#   candidate/<name>.exit
#
# The loader-interface court has no corpus: both CLIs render the fixed
# interface once.
#
# Then: cargo xtask court run scx/<surface>
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ORACLE_BIN="$ROOT/target/release/scx-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-scx-model"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p scx-oracle-ref -p cachyos-kernel-manager-scx --bins

run_one() { # run_one <bin> <surface> <outdir> <file>
    local bin="$1" surface="$2" outdir="$3" file="$4"
    local name; name="$(basename "$file" .json)"
    local stdout rc
    if stdout="$("$bin" "$surface" parse "$file" 2>/dev/null)"; then
        rc=0
    else
        rc=$?
    fi
    printf '%s' "$stdout" > "$outdir/$name.json"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for surface in button-visibility current-scheduler mode-flags window-init profile apply disable; do
    case_dir="$ROOT/courts/scx/$surface"
    [ -d "$case_dir/fixture/corpus" ] || continue
    mkdir -p "$case_dir/oracle" "$case_dir/candidate"
    for f in "$case_dir"/fixture/corpus/*.json; do
        run_one "$ORACLE_BIN" "$surface" "$case_dir/oracle" "$f"
        run_one "$CANDIDATE_BIN" "$surface" "$case_dir/candidate" "$f"
    done
done

# loader-interface: no corpus — the interface is fixed by the type system
case_dir="$ROOT/courts/scx/loader-interface"
mkdir -p "$case_dir/oracle" "$case_dir/candidate"
"$ORACLE_BIN" interface parse /dev/null > "$case_dir/oracle/interface.json" 2>/dev/null || true
printf '%d' "$?" > "$case_dir/oracle/interface.exit"
"$ROOT/target/release/cachyos-kernel-manager-scx-introspect" > "$case_dir/candidate/interface.json" 2>/dev/null || true
printf '%d' "$?" > "$case_dir/candidate/interface.exit"

echo "scx corpus witness written to courts/scx/*"
echo "compare: cargo xtask court run scx/<surface>"
