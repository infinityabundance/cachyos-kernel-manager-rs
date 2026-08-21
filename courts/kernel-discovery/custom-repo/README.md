# kernel-discovery/custom-repo

fake kernel only in the [fixtures] repo, not installed

Status: defined. Execution: `cargo xtask court run kernel-discovery/custom-repo --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake custom-repo`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
