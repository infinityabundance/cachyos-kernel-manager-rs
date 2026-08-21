# kernel-discovery/duplicate-across-repos

same fake kernel name in [fixtures] and [other] with different versions

Status: defined. Execution: `cargo xtask court run kernel-discovery/duplicate-across-repos --vm`
(requires the baked base image and this fixture: `cargo xtask vm bake duplicate-across-repos`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
