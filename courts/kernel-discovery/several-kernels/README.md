# kernel-discovery/several-kernels

real kernels from core/extra/cachyos installed (linux, linux-lts, linux-zen, linux-cachyos-lts, linux-cachyos-rt-bore)

Status: defined. Execution: `cargo xtask court run kernel-discovery/several-kernels --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake several-kernels`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
