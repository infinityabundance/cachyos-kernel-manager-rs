# kernel-discovery/adversarial-names

headers-without-kernel, kernel-without-headers, linux-api-headers skip, non-kernel linux-ish packages.

Status: defined. Execution: `cargo xtask court run kernel-discovery/adversarial-names --vm`
(requires the baked fixture: `cargo xtask vm bake adversarial-names`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
