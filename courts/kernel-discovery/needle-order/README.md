# kernel-discovery/needle-order

gap-002 court: the alpm_db_search ordering within a sync db. The fixture is
an **adversarial needle db** (`vm/fixtures/needle-order`): 40 interleaved
kernel/headers families (kA0..kD9, headers inserted before their kernels so
the pkgcache holds them interleaved) plus the traps:

- `linux-api-headers` + `linux-api-headers-dev` — skipped (the
  `linux-api-headers` substring rule, kernel.cpp:199-204);
- `linux-lonely-headers` (headers WITHOUT a kernel) — skipped;
- `linux-orphan` (kernel WITHOUT headers) — invisible (discovery is driven
  by the headers needle);
- `notlinux-headers` — the needle requires the `linux` prefix;
- `linux`/`linux-headers` and `linux-lts`/`linux-lts-headers` — the bare
  kernel families (the oracle's `linux`-specific companion rules apply).

The claim: the row SET and the row ORDER are identical between the oracle's
AT-SPI tree (alpm_db_search) and the candidate's inspect state
(alpm_db_get_pkgcache) — both iterate the same libalpm pkgcache hash on the
same libalpm build. The order is NOT an ALPM contract (it may change across
libalpm versions — gap-002); this court pins the equivalence on the frozen
oracle's libalpm.

Status: defined. Execution:

```
cargo xtask vm bake needle-order
cargo xtask court run kernel-discovery/needle-order --vm
```

Falsifier: any difference in row set, order, version text, category, or
checked state.
