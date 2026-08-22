# custom-name/pkgbase-injection

Phase 6 mutation court on fixture `build-mutation`: the honest differential
for the PKGBUILD `pkgbase="..."` insertion (`conf-window.cpp:328-339`).

The oracle side launches the REAL GUI (cwd = the pkgbuilds cache) under
strace and drives the Configure window through AT-SPI (`oracle-mutate.py`):
click Configure, replace the custom-name entry with `my-kernel`, snapshot
the PKGBUILD, click Build kernel, snapshot again. The candidate side runs
`cachyos-kernel-manager-mutate` against the SAME fixture PKGBUILD (fresh
overlay) with the same custom name. The comparator byte-compares the
pre-mutation PKGBUILDs (fixture-integrity) and the post-mutation PKGBUILDs
(the pkgbase + splice residual).

Actions: custom name `my-kernel`, no remote patch added.

Run: `cargo xtask vm bake build-mutation && cargo xtask court run
custom-name/pkgbase-injection --vm`

Falsifier: any byte difference in the mutated PKGBUILD or the pre-mutation
text, any discovery row difference, or machine residual drift.
