# git-cache/lifecycle

Phase 6 configure-flow court on fixture `git-cache` — the honest differential
for the `prepare_git_repo` lifecycle (`utils.cpp:161-202`).

The oracle side launches the REAL GUI under strace and clicks the Configure
button through AT-SPI (`oracle-configure.py`), which triggers
`prepare_build_environment` in a background worker; the strace witness
captures the git refresh chain (`git checkout --force master`,
`git clean -fd`, `git pull`). The candidate side runs
`cachyos-kernel-manager-gitcache`, which probes the identical fixture
filesystem and emits the model's predicted chain. The comparator compares
the exec chains witness-by-witness
(`oracle/oracle-transaction.json` vs `candidate/candidate-transaction.json`),
the discovery rows, and the machine residual.

Fixture `git-cache`: `/root/.cache/cachyos-km/pkgbuilds` is a git checkout
(branch master, clean) whose origin is a LOCAL bare remote
(`/root/cachyos-km-remote.git`) one commit ahead — the refresh chain runs
fully offline and actually fast-forwards.

Run: `cargo xtask vm bake git-cache && cargo xtask court run git-cache/lifecycle --vm`

Falsifier: any difference in git exec order or argv, any discovery row
difference, or machine residual drift.
