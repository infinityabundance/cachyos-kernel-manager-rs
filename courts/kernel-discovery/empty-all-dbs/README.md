# kernel-discovery/empty-all-dbs

no sync databases at all; oracle must show the No kernels found dialog

Status: defined. Execution: `cargo xtask court run kernel-discovery/empty-all-dbs --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake empty-all-dbs`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
