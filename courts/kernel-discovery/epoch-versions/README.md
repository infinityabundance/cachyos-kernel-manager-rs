# kernel-discovery/epoch-versions

epoch and unusual Arch version syntax in display + upgrade/downgrade markers.

Status: defined. Execution: `cargo xtask court run kernel-discovery/epoch-versions --vm`
(requires the baked fixture: `cargo xtask vm bake epoch-versions`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
