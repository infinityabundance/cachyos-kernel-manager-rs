# pacman-config/duplicated-sections

duplicated [fixtures] section merges into one registration (real pacman errors).

Status: defined. Execution: `cargo xtask court run pacman-config/duplicated-sections --vm`
(requires the baked fixture: `cargo xtask vm bake duplicated-sections`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
