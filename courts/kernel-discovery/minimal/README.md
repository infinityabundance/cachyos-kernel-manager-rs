# kernel-discovery/minimal

base state: linux-cachyos installed from cachyos, all sync dbs as built

Status: defined. Execution: `cargo xtask court run kernel-discovery/minimal --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake minimal`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
