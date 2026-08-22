# aur/enablement-matrix

Non-VM differential court for the oracle's AUR support (`kernel.cpp:253-283`
discovery, `89-95` install routing, `288-304` commit_transaction,
`aur_kernel.cpp:32-55`, `string_utils.hpp:48-72`), over a frozen 7-case
corpus:

| corpus file | exercises |
|---|---|
| `shipped-oracle.json` | the SHIPPED binary: flag OFF, paru+awk installed, AUR packages listed → no probe, no rows, no message |
| `flag-on-full.json` | flag ON: rows parsed/stripped/deduped (vs repo and vs earlier AUR rows), AUR-first commit order, pacman last |
| `flag-on-paru-missing.json` | flag ON, `/sbin/paru` missing → the `!paru \|\| !awk` gate message (5b075dc flip), no probe |
| `flag-on-awk-missing.json` | flag ON, `/sbin/awk` missing → identical gate (EITHER missing disables) |
| `flag-on-empty-kernels.json` | flag ON, no repo kernels → the `!kernels.empty()` probe gate, no message |
| `flag-on-headers-skip.json` | the `headers`-substring build skip: the row exists, its build is skipped |
| `flag-on-multi-aur-order.json` | selection order preserved in the commit chain; mixed install+remove |
| `flag-on-empty-commit.json` | rows exist, no selections → empty commit |

The meson-vs-CMake difference is central: `ENABLE_AUR_KERNELS` is a meson
`aur_kernels` option (default off); the SHIPPED CMake build does not define
it, so the frozen binary's AUR path is compiled out and cannot be witnessed
in the VM. This court therefore:
- witnesses the **disabled** side against the shipped binary's source-derived
  behavior (flag off → nothing happens even with paru installed), and
- witnesses the **enabled** side against `tools/aur-oracle-ref`, which
  re-declares the exact decisions from the frozen source.

The gate message goes to stderr and is compared byte-exact (it is a
user-visible contract, reworded at upstream commit 5b075dc).

Witness: `tools/run-aur-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json`/`.stderr`/`.exit` + candidate equivalents);
`cargo xtask court run aur/enablement-matrix` byte-compares them.

Status: defined. Run:

```
tools/run-aur-corpus.sh
cargo xtask court run aur/enablement-matrix
```

Falsifier: any byte difference in any output file (model JSON, stderr, exit
code) on any corpus case.
