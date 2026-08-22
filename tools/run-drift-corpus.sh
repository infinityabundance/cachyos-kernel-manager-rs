#!/usr/bin/env bash
#
# run-drift-corpus.sh — witness runner for the drift-slew/pure-determinism
# court (Phase 9).
#
# Runs EVERY pure corpus-driven witness THREE times over its frozen corpus
# (fresh processes each run) and records:
#   oracle/<witness>/<name>.json       run 1
#   candidate/<witness>/<name>.json    run 2
#   (run 3 is diffed against run 1 inside this runner — a hard failure if
#   ANY byte differs: the court's own 3rd sample is the runner's assert)
#
# Determinism across fresh processes is the claim; any drift is a residual
# (oracle/ vs candidate/ differ) AND a runner failure (run3 != run1).
#
# Then: cargo xtask court run drift-slew/pure-determinism
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/drift-slew/pure-determinism"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

# witness list: bin|corpus-dir
PARSE_WITNESSES=(
  "cachyos-kernel-manager-config|config-roundtrip/canonicalization"
  "cachyos-kernel-manager-variant-switch|option-transitions/variant-switch"
  "cachyos-kernel-manager-buildflow|build-env/lifecycle"
  "cachyos-kernel-manager-env|build-env/env-rendering"
  "cachyos-kernel-manager-mainwindow|ui/main-window-semantics"
  "cachyos-kernel-manager-confwindow|ui/configure-window-semantics"
  "cachyos-kernel-manager-i18n|ui/i18n-resolution"
  "cachyos-kernel-manager-single-instance|single-instance/stale-lock"
)
# the fixed (no-corpus) witnesses: render once per run
FIXED_WITNESSES=(
  "cachyos-kernel-manager-strings"
)

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p cachyos-kernel-manager-config -p cachyos-kernel-manager-build \
    -p cachyos-kernel-manager-ui --features rendering \
    -p cachyos-kernel-manager-platform --bins

run_one() { # run_one <bin> <outdir> <corpus-file>
    local bin="$1" outdir="$2" f="$3"
    mkdir -p "$outdir"
    local name; name="$(basename "$f" .json)"
    local out rc=0
    out="$("$ROOT/target/release/$bin" parse "$f" 2>/dev/null)" || rc=$?
    printf '%s' "$out" > "$outdir/$name.json"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

run_parse_set() { # run_parse_set <bin> <corpus_dir> <outdir>
    local bin="$1" corpus_dir="$2" outdir="$3"
    mkdir -p "$outdir"
    for f in "$ROOT/courts/$corpus_dir"/fixture/corpus/*.json; do
        run_one "$bin" "$outdir" "$f"
    done
}

rm -rf "$ORACLE" "$CANDIDATE"

for entry in "${PARSE_WITNESSES[@]}"; do
    IFS='|' read -r bin corpus_dir <<< "$entry"
    run_parse_set "$bin" "$corpus_dir" "$ORACLE/$bin"
    run_parse_set "$bin" "$corpus_dir" "$CANDIDATE/$bin"
    # run 3: diffed against run 1 (the runner's own determinism assert)
    local_tmp="$(mktemp -d)"
    run_parse_set "$bin" "$corpus_dir" "$local_tmp"
    if ! diff -r "$ORACLE/$bin" "$local_tmp" >/dev/null; then
        echo "DRIFT: $bin run3 differs from run1"
        rm -rf "$local_tmp"
        exit 1
    fi
    rm -rf "$local_tmp"
done

for bin in "${FIXED_WITNESSES[@]}"; do
    mkdir -p "$ORACLE/$bin" "$CANDIDATE/$bin"
    for run in 1 2 3; do
        out="$("$ROOT/target/release/$bin" 2>/dev/null)" || rc=$?
        printf '%s' "$out" > "/tmp/drift-$bin-$run.json"
        printf '%d' "${rc:-0}" > "/tmp/drift-$bin-$run.exit"
        rc=0
    done
    cp "/tmp/drift-$bin-1.json" "$ORACLE/$bin/table.json"
    cp "/tmp/drift-$bin-1.exit" "$ORACLE/$bin/table.exit"
    cp "/tmp/drift-$bin-2.json" "$CANDIDATE/$bin/table.json"
    cp "/tmp/drift-$bin-2.exit" "$CANDIDATE/$bin/table.exit"
    if ! cmp -s "/tmp/drift-$bin-1.json" "/tmp/drift-$bin-3.json"; then
        echo "DRIFT: $bin run3 differs from run1"
        rm -f "/tmp/drift-$bin-"*.json "/tmp/drift-$bin-"*.exit
        exit 1
    fi
    rm -f "/tmp/drift-$bin-"*.json "/tmp/drift-$bin-"*.exit
done

echo "drift-slew witness written to courts/drift-slew/pure-determinism/{oracle,candidate}"
echo "compare: cargo xtask court run drift-slew/pure-determinism"
