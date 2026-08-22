# pacman-config/testing-and-disabled

[testing] skipped, [core-testing] registered, commented repos not registered.

Status: defined. Execution: `cargo xtask court run pacman-config/testing-and-disabled --vm`
(requires the baked fixture: `cargo xtask vm bake testing-and-disabled`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
