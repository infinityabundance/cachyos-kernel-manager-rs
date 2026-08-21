# kernel-discovery/empty-sync-db

[emptyrepo] sync db present with zero packages

Status: defined. Execution: `cargo xtask court run kernel-discovery/empty-sync-db --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake empty-sync-db`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
