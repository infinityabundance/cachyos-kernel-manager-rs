# kernel-discovery/downgrade-visible

fake kernel: local 9.9.9 > sync 9.8.8 (∨ marker, no update flag)

Status: defined. Execution: `cargo xtask court run kernel-discovery/downgrade-visible --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake downgrade-visible`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
