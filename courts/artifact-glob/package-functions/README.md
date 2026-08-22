# artifact-glob/package-functions

Non-VM differential court for artifact discovery (§26): the candidate's
`parse_pkgfuncs_probe_output` + `artifact_globs` (build crate) vs the
oracle's `get_package_names_glob_from_pkgbuild` + `prepare_func_names`
(`conf-window.cpp:274-298,238-272`), over 8 fixture PKGBUILDs × 4 pkgext
cases:

| PKGBUILD | exercises |
|---|---|
| `linux-cachyos` | the real split-package shape (headers/docs suffixes) |
| `single` | one package function |
| `broken-pkgver` | no `pkgver: ` line -> error + empty globs |
| `bare-package-fn` | bare `package()` dropped by the `package_` filter |
| `no-package-fns` | only prepare/build -> empty globs, no error |
| `epoch` | epoch in pkgver (`1:9.9.9-1`) |
| `weird-suffix` | hyphenated suffixes preserved whole |
| `git-cache-shape` | the build-mutation fixture PKGBUILD shape |

pkgext cases: `probe` (real /etc/makepkg.conf), `probe-empty` (the
`.pkg.tar.zst` fallback), `.pkg.tar.zst`, `.pkg.tar.xz`.

Both sides execute the SAME probe scripts (bash is the contract) against
the SAME frozen PKGBUILDs, so the comparison targets the parse/glob
algorithms. Host safety: the fixture PKGBUILDs are static and benign.

Witness: `tools/run-artifact-corpus.sh`; `cargo xtask court run
artifact-glob/package-functions`.

Status: defined. Run:

```
tools/run-artifact-corpus.sh
cargo xtask court run artifact-glob/package-functions
```

Falsifier: any difference in probe output, suffixes, pkgver string, error
condition, or globs on any case.
