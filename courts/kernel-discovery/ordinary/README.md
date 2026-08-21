# kernel-discovery/ordinary

Proves the discovery contract against a controlled package database in a
disposable VM: the oracle (real CachyOS Kernel Manager v1.19.0) and the
candidate run against the same frozen fixture; both serialize
`(repo, kernel, headers, companions)` state; the comparator fingerprints the
directories.

## Status

Defined. Execution requires the Phase 2 VM harness (`cargo xtask vm build`).
Pure-model unit coverage already lives in
`crates/cachyos-kernel-manager-core/src/discovery.rs` — that is NOT this
court; this court is the differential proof.

## Falsifier

Any difference in the tuple list, companion resolution, or row order.
