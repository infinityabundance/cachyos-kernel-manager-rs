# pacman-config/missing-conf

/etc/pacman.conf absent: zero registrations, empty discovery, No kernels found dialog.

Status: defined. Execution: `cargo xtask court run pacman-config/missing-conf --vm`
(requires the baked fixture: `cargo xtask vm bake missing-conf`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
