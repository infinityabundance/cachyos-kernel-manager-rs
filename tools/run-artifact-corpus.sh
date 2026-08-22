#!/usr/bin/env bash
#
# run-artifact-corpus.sh — witness runner for the
# artifact-glob/package-functions court.
#
# For every fixture PKGBUILD x every pkgext case, run the oracle reference
# CLI (conf-window.cpp:218-298 pipeline, executing the REAL probe scripts
# via bash — bash is the contract) and the candidate CLI (the candidate's
# parse/glob models over the same probe outputs), and record:
#   oracle/<pkg>-<pkgext>.json
#   oracle/<pkg>-<pkgext>.exit
#   candidate/<pkg>-<pkgext>.json
#   candidate/<pkg>-<pkgext>.exit
#
# Host safety: the fixture PKGBUILDs are static, benign files checked into
# the repo; the probe scripts only source them and echo function names.
# Then: cargo xtask court run artifact-glob/package-functions
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/artifact-glob/package-functions"
PKGS="$CASE/fixture/pkgs"
PKGEXT_CASES="$CASE/fixture/pkgext-cases.json"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

ORACLE_BIN="$ROOT/target/release/artifact-oracle-ref"
CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-artifact-glob"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p artifact-oracle-ref -p cachyos-kernel-manager-build --bins

mkdir -p "$ORACLE" "$CANDIDATE"

mapfile -t PKGEXTS < <(python3 -c "import json,sys; print('\\n'.join(json.load(open('$PKGEXT_CASES'))))")

run_one() { # run_one <bin> <outdir> <pkgbuild> <pkgext> <name>
    local bin="$1" outdir="$2" pkgbuild="$3" pkgext="$4" name="$5"
    local stdout rc
    if stdout="$("$bin" probe "$pkgbuild" "$pkgext" 2>/dev/null)"; then
        rc=0
    else
        rc=$?
    fi
    printf '%s' "$stdout" > "$outdir/$name.json"
    printf '%d' "$rc" > "$outdir/$name.exit"
}

for pkg in "$PKGS"/*.PKGBUILD; do
    pkgbase="$(basename "$pkg" .PKGBUILD)"
    for i in "${!PKGEXTS[@]}"; do
        pe="${PKGEXTS[$i]}"
        name="${pkgbase}-pkgext$i"
        run_one "$ORACLE_BIN" "$ORACLE" "$pkg" "$pe" "$name"
        run_one "$CANDIDATE_BIN" "$CANDIDATE" "$pkg" "$pe" "$name"
    done
done

echo "artifact corpus witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run artifact-glob/package-functions"
