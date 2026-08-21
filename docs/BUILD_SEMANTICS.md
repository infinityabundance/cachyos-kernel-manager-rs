# BUILD SEMANTICS

Reconstructed build subsystem (revision `6b4a373e`); implementation lives in
`crates/cachyos-kernel-manager-build` (pure) and `crates/cachyos-kernel-manager-exec`
(process boundary, Phase 5). Courts: `patch-injection/*`, `custom-name/*`,
`build-env/*`, `artifact-glob/*`.

## Environment

`get_all_set_values` renders one line per option: `_<var>=<value>\n`; 11
checkboxes always emit yes/no; combos emit `_HZ_ticks`, `_tickrate`,
`_preempt`, `_hugepage`, `_lto`; `_processor_opt` only when != manual;
`_use_lto_suffix=n` when lto != none and custom name != `$pkgbase` (workaround
for PKGBUILD custom-pkgname breakage). Vars are `setenv` into the app process
and unset before the next build (`restore_clean_environment`).

## Probes (bash is the contract)

- source array: `.testscript` — sources PKGBUILD with the env lines in
  scope, echoes `${source[@]}`.
- PKGEXT: `.testscriptpkgext` — sources `/etc/makepkg.conf`, echoes
  `${PKGEXT}` (fallback `.pkg.tar.zst`).
- functions: `.testscriptpkgnames` — `declare -F` + `pkgver: $pkgver-$pkgrel`.

## PKGBUILD mutation

- `source=(...)`: original entries minus `*.patch`, then patch-list entries,
  each quoted `"..."`, joined `\n`, rendered `source=(\n...)\n`, inserted at
  the last newline before `prepare()`. Original block untouched (later
  assignment wins). Silent no-op when `prepare()` is absent (build proceeds
  without the user's patches — preserved).
- `pkgbase="<custom>"`: `\n\n` + value, inserted at the last newline before
  `_major=`. Silent no-op when `_major=` is absent or at file start.
- Writes are non-atomic in the oracle (truncate in place). The candidate
  uses atomic replace (D-002, documented divergence) while producing
  byte-identical content residuals.

## Build + artifact lifecycle

`makepkg -scf --cleanbuild --skipchecksums && touch .done-status` in a
non-escalated terminal; success = `.done-status` present (NOT exit code).
On success: prompt, then `sudo pacman -U <globs>` where each glob is
`<package_<suffix>-stripped>-<pkgver>-<pkgrel>-*<PKGEXT>`.

## Option transitions

Variant switches reset lto (thin-dist availability; thin default for
cachyos/rc else none), preempt (Voluntary/None only for lts/hardened; lazy
default for server), hz (300 server else 1000), cachyconfig (unchecked for
server), builtin_zfs (disabled+unchecked for rt), then refresh the patches
tab. See `core::options::VariantTransitions`.
