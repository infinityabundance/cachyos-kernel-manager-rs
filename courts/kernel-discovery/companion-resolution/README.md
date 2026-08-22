# kernel-discovery/companion-resolution

zfs/nvidia/nvidia-open companion presence per kernel (source-anchored model).

Status: defined. Execution: `cargo xtask court run kernel-discovery/companion-resolution --vm`
(requires the baked fixture: `cargo xtask vm bake companion-resolution`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
