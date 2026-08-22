# pacman-config/malformed

malformed pacman.conf: [a=b key, unclosed [broken, stray text, numeric auto-sections.

Status: defined. Execution: `cargo xtask court run pacman-config/malformed --vm`
(requires the baked fixture: `cargo xtask vm bake malformed`).

Falsifier: any difference in row set, order, version text, category, checked
state, or the empty-discovery dialog condition.
