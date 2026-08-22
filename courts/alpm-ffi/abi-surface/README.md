# alpm-ffi/abi-surface

Non-VM ABI court for the hand-written libalpm FFI (`src/ffi.rs`). The two
historical bugs — the list-layout OOM (RES-2026-002: `RawList` missing the
`prev` field) and the `installed_db` SIGSEGV (RES-2026-003: wrong return
type) — were both hand-reconstructed C ABI facts. This court machine-verifies
every such fact:

- oracle side: `abi/probe.c` compiled with `-Werror` against the ACTUAL
  headers (`_Static_assert`s for `alpm_list_t` layout + enum sizes +
  `ALPM_SIG_USE_DEFAULT`; function-pointer assignments for every extern
  declaration — a signature drift is a compile error), then run to print
  the constants;
- candidate side: `cachyos-kernel-manager-alpm-abi` prints the Rust side's
  ACTUAL compiled constants (`size_of`/`offset_of` over `RawList`).

The same probe is compiled and run by `build.rs` at build time (a drift
panics the build before the FFI can link). This court is the evidence
record: `tools/run-abi-probe.sh` → `cargo xtask court run alpm-ffi/abi-surface`.

Falsifier: any byte difference in the printed constants, or a nonzero probe
exit.
