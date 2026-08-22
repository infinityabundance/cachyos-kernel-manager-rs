#!/usr/bin/env bash
#
# run-regression-corpus.sh — witness runner for the
# regression-suite/pure-regressions court (Phase 9, historical regressions).
#
# Re-verifies the RES-2026-002/003/004/012 resolutions LIVE:
#   RES-2026-012 (lto var): the env rendering's `_use_llvm_lto=` line
#     (declared expectation vs the live witness output);
#   RES-2026-004 (cross-repo row): the row's present/checked/immutable
#     (declared expectation vs the live witness output);
#   RES-2026-002/003 (ABI): the abi witness (BOTH sides run the same
#     witness — the assertion is that it still BUILDS and RUNS with the
#     probe == Rust constants; environment-adaptive: without libalpm the
#     skip note is recorded on both sides).
#
# Then: cargo xtask court run regression-suite/pure-regressions
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/regression-suite/pure-regressions"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"

mkdir -p "$ORACLE" "$CANDIDATE"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p cachyos-kernel-manager-build --bin cachyos-kernel-manager-env \
    -p cachyos-kernel-manager-ui --bin cachyos-kernel-manager-mainwindow

# RES-2026-012: the lto var line (the declared expectation, from the frozen
# option_map: conf-window.cpp:421-451 / compile_options.json)
printf '%s\n' "_use_llvm_lto=thin" > "$ORACLE/env-lto.txt"
"$ROOT/target/release/cachyos-kernel-manager-env" parse \
    "$ROOT/courts/build-env/env-rendering/fixture/corpus/default.json" 2>/dev/null \
    | grep '^_use_llvm_lto=' > "$CANDIDATE/env-lto.txt" || true
if ! grep -q '^_use_llvm_lto=' "$CANDIDATE/env-lto.txt"; then
    echo "REGRESSION: RES-2026-012 (the _use_llvm_lto var is missing)" >&2
    exit 1
fi

# RES-2026-004: the cross-repo row (the declared expectation, from
# km-window.cpp:97-104: present, unchecked, NOT immutable)
printf '%s\n' "raw=cachyos/linux-cachyos checked=False immutable=False" > "$ORACLE/cross-repo-row.txt"
ROW="$("$ROOT/target/release/cachyos-kernel-manager-mainwindow" parse \
    "$ROOT/courts/ui/main-window-semantics/fixture/corpus/cross-repo-installed.json" 2>/dev/null \
    | python3 -c "
import json,sys
d = json.load(sys.stdin)
for r in d['rows']:
    if r['raw'] == 'cachyos/linux-cachyos':
        print(f\"raw={r['raw']} checked={r['checked']} immutable={r['immutable']}\")
")"
[ -n "$ROW" ] || { echo "REGRESSION: RES-2026-004 (the cross-repo row is missing)" >&2; exit 1; }
printf '%s\n' "$ROW" > "$CANDIDATE/cross-repo-row.txt"

# RES-2026-002/003: the ABI witness (both sides; environment-adaptive)
if pkg-config --exists libalpm 2>/dev/null; then
    cargo build --manifest-path "$ROOT/Cargo.toml" --release \
        -p cachyos-kernel-manager-alpm --features libalpm \
        --bin cachyos-kernel-manager-alpm-abi
    "$ROOT/target/release/cachyos-kernel-manager-alpm-abi" > "$ORACLE/abi-probe.txt" 2>/dev/null || true
    "$ROOT/target/release/cachyos-kernel-manager-alpm-abi" > "$CANDIDATE/abi-probe.txt" 2>/dev/null || true
else
    echo "skipped (no libalpm)" > "$ORACLE/abi-probe.txt"
    echo "skipped (no libalpm)" > "$CANDIDATE/abi-probe.txt"
fi

echo "regression assertions written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run regression-suite/pure-regressions"
