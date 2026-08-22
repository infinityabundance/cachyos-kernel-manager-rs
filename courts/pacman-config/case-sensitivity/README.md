# pacman-config/case-sensitivity

[Fixtures] section lowercased by mINI and discovers fixtures.db (real pacman would not).

Status: defined. Execution: `cargo xtask court run pacman-config/case-sensitivity --vm`
(requires the baked fixture: `cargo xtask vm bake case-sensitivity`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
