#!/usr/bin/env bash
#
# run-abi-probe.sh — witness runner for the alpm-ffi/abi-surface court.
#
# Oracle side: compile abi/probe.c against the ACTUAL installed libalpm
# headers (-Werror: static asserts + function-pointer signature checks) and
# run it — the C reality of every ABI fact the handwritten FFI assumes.
# Candidate side: cachyos-kernel-manager-alpm-abi — the Rust side's ACTUAL
# compiled layout constants in the same format.
#
# Records:
#   oracle/abi.txt      the probe output
#   oracle/abi.exit     probe exit code (0 = all asserts held)
#   candidate/abi.txt   the Rust-side constants
#   candidate/abi.exit
#
# Then: cargo xtask court run alpm-ffi/abi-surface
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASE="$ROOT/courts/alpm-ffi/abi-surface"
ORACLE="$CASE/oracle"
CANDIDATE="$CASE/candidate"
PROBE="$ROOT/crates/cachyos-kernel-manager-alpm/abi/probe.c"

CANDIDATE_BIN="$ROOT/target/release/cachyos-kernel-manager-alpm-abi"

cargo build --manifest-path "$ROOT/Cargo.toml" --release \
    -p cachyos-kernel-manager-alpm --features libalpm --bin cachyos-kernel-manager-alpm-abi

mkdir -p "$ORACLE" "$CANDIDATE"

# --- oracle side: the C probe against the real headers ---
INCFLAGS="$(pkg-config --cflags libalpm)"
CC="${CC:-cc}"
PROBE_BIN="$(mktemp)"
ORACLE_OUT=""
if "$CC" -Werror $INCFLAGS "$PROBE" -lalpm -o "$PROBE_BIN" 2>/dev/null && ORACLE_OUT="$("$PROBE_BIN" 2>/dev/null)"; then
    ORACLE_RC=0
else
    ORACLE_RC=$?
fi
rm -f "$PROBE_BIN"
printf '%s' "$ORACLE_OUT" > "$ORACLE/abi.txt"
printf '%d' "$ORACLE_RC" > "$ORACLE/abi.exit"

# --- candidate side: the Rust layout constants ---
CAND_OUT=""
if CAND_OUT="$("$CANDIDATE_BIN" 2>/dev/null)"; then
    CAND_RC=0
else
    CAND_RC=$?
fi
printf '%s' "$CAND_OUT" > "$CANDIDATE/abi.txt"
printf '%d' "$CAND_RC" > "$CANDIDATE/abi.exit"

echo "abi witness written to $ORACLE and $CANDIDATE"
echo "compare: cargo xtask court run alpm-ffi/abi-surface"
