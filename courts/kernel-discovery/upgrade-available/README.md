# kernel-discovery/upgrade-available

fake kernel: local 9.8.8 < sync 9.9.9 (∧ marker, update flag)

Status: defined. Execution: `cargo xtask court run kernel-discovery/upgrade-available --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake upgrade-available`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
