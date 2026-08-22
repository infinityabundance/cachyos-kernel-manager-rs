# patch-injection/source-array

Phase 6 mutation court on fixture `build-mutation`: the honest differential
for the PKGBUILD `source=(...)` splice (`conf-window.cpp:300-326`).

The oracle side launches the REAL GUI (cwd = the pkgbuilds cache) under
strace and drives the Configure window through AT-SPI
(`oracle-mutate.py`): click Configure, answer the "Add remote patch"
QInputDialog with `https://example.invalid/custom.patch`, leave the custom
name at the window default, snapshot the PKGBUILD, click Build kernel,
snapshot again. The candidate side runs `cachyos-kernel-manager-mutate`
against the SAME fixture PKGBUILD (fresh overlay) with the same additions.
The comparator byte-compares the pre-mutation PKGBUILDs (fixture-integrity)
and the post-mutation PKGBUILDs (the splice residual).

Actions: patch `https://example.invalid/custom.patch`, custom name left at
`$pkgbase-custom`.

Run: `cargo xtask vm bake build-mutation && cargo xtask court run
patch-injection/source-array --vm`

Falsifier: any byte difference in the mutated PKGBUILD or the pre-mutation
text, any discovery row difference, or machine residual drift.
