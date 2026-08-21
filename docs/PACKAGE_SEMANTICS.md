# PACKAGE SEMANTICS

How the oracle treats packages — the facts the ALPM layer and courts must
preserve (revision `6b4a373e`).

## Version comparison

ALPM version semantics everywhere (`alpm_pkg_vercmp`), never semver. Version
strings are Arch `epoch:pkgver-pkgrel` forms. The candidate's version-state
computation (`core::kernel::DisplayVersion`) is comparator-agnostic; the
comparator is supplied by the ALPM layer.

## Kernel identity

- Display name = `<repo>/<kernel>` (the "raw" form). Package name = bare
  name. A kernel is *the same kernel* across repos only by bare name; rows
  are per-repo.
- Headers pairing is strictly same-database.
- Installed provenance (`alpm_pkg_get_installed_db`) changes row semantics:
  installed-from-same-repo → immutable+checked; installed-from-other-repo →
  present but mutable+unchecked.

## Companion modules

- linux-cachyos*: `<pkg>-zfs`, `<pkg>-nvidia`, `<pkg>-nvidia-open`
  (same db).
- linux/linux-lts: `nvidia[-lts]`, `nvidia-open[-lts]` (same db; all
  `linux` substrings stripped).
- No companions for other kernels (e.g. linux-zen).

## Transaction rendering

- Install: `pacman -S --needed <list>` — list order `[zfs?, nvidia?,
  kernel, headers]` per selected kernel, in selection order. Packages never
  come from user text (except validated custom pkgbase, which never reaches
  `pacman -S`).
- Remove: `pacman -Rsn <list>` — kernel, then installed-only companions
  (headers, zfs, nvidia, nvidia-open).
- AUR kernels: `makepkg -sicf --cleanbuild --skipchecksums` per kernel
  (no companions), first in commit order.

## Version display

- AUR: `unknown-version`.
- Installed + local newer: `∨<local>` (downgrade).
- Installed + sync newer: `∧<sync>` (update flag set).
- Equal or not installed: plain sync version.

## Court coverage

`transaction-plan/*`, `version-state/*`, `kernel-discovery/*`,
`nvidia-companion/*`, `zfs-companion/*` — see `atlas/court-ledger.json`.
