# COMPATIBILITY

The drop-in replacement contract, derived from the frozen oracle
(`atlas/inventory.json` is the machine-readable form). Classification legend:

- `REQUIRED_PARITY` — must match, courted.
- `COMPATIBILITY_QUIRK` — observable oddity; preserved unless corrected with
  evidence.
- `SECURITY_CORRECTION` — deliberate divergence with full documentation.
- `INTENTIONAL_CORRECTION` — deliberate divergence for correctness.

## Identities that must not change

| surface | value |
|---|---|
| binary | `cachyos-kernel-manager` |
| desktop file | `org.cachyos.KernelManager.desktop` (`Name=CachyOS Kernel Manager`, `Categories=Qt;System;`, `X-AppStream-Ignore=true`) |
| icon | `org.cachyos.KernelManager` (hicolor 16–310) |
| org/app | `CachyOS` / `cachyos.org` / `CachyOS-KM` / desktop-file-name `org.cachyos.KernelManager` |
| single-instance lock | key `CachyOS-KM-lock`; second instance exit `-1` |
| polkit action | `org.cachyos.KernelManager.pkexec.policy.run-root-terminal`, `auth_admin`, annotated exec `/usr/lib/cachyos-kernel-manager/rootshell.sh` |
| helper dir | `lib/cachyos-kernel-manager/` (`terminal-helper`, `rootshell.sh`) |
| cache root | `~/.cache/cachyos-km` (`pkgbuilds/`, `aur_pkgbuilds/`) |
| config schema | the 18-field TOML from `config-option-lib` (field order preserved) |

The desktop `Categories=Qt` entry is a **historical implementation artifact**
(Qt is gone in the candidate). It is retained in the packaged .desktop only
if drop-in packaging requires byte parity; otherwise it is deliberately
reclassified and documented (per directive §32). This is recorded in the
court ledger as `desktop.categories`.

## Behavioral contracts (courted)

- Discovery: sync-db iteration in pacman.conf order, `linux[^ ]*-headers`
  regex search, `linux-api-headers` exclusion, same-db kernel/headers
  pairing, `db/name` display, installed-db provenance, AUR merge when
  enabled.
- Version display: `∨`/`∧` markers with ALPM vercmp; `unknown-version` for
  AUR; version-column sort strips markers and uses vercmp.
- Category labels: exact string set from the classifier.
- Companion expansion: linux-cachyos zfs/nvidia/nvidia-open + linux/linux-lts
  nvidia[-lts]/nvidia-open[-lts], same-db only.
- ZFS: `findmnt -ln -o FSTYPE /` == `zfs`.
- NVIDIA decision matrix (see archaeology doc) incl. chwd profile sniffing,
  DKMS suppression, installed-module precedence, open-over-closed.
- Install order `[zfs?, nvidia?, kernel, headers]`; `pacman -S --needed`;
  remove `pacman -Rsn` with installed-only companions.
- Terminal-helper priority list and behaviors; `$TERMINAL` ignored; kgx
  SIGQUIT hack; notify-send fallback; exit-code quirks.
- Escalation chain `terminal-helper -s pkexec <rootshell> <cmd>` with
  `; read -p 'Press enter to exit'` appended.
- `popen`-style shell exec semantics for probes (findmnt/chwd/pacman -Qqs/
  paru/testscripts), single trailing newline strip, `-1` on popen failure.
- Configure window: variant list/labels, option value lists, defaults,
  variant-switch resets, env var rendering incl. `_use_lto_suffix=n`
  workaround, patches tab rules, local `file://` prefix, remote URL dialog,
  reorder/remove.
- PKGBUILD mutation: `source=(...)` insertion before `prepare()`, `pkgbase`
  insertion before `_major=`, both non-atomic plain writes (candidate adds
  atomic replace as `INTENTIONAL_CORRECTION` documented in
  KNOWN_DIVERGENCES).
- Build: `makepkg -scf --cleanbuild --skipchecksums && touch .done-status`;
  success = marker file; artifact globs from `package_*` functions +
  `pkgver-pkgrel` + PKGEXT probe; `sudo pacman -U` prompt on success.
- Dialogs, stdout/stderr strings, and progress text (exact strings in
  atlas/inventory.json).
- SCX button visibility (`/sys/kernel/sched_ext/state`), scxctl-ui window
  behavior.

## Divergence policy

Every divergence is either:

1. classified and courted (with `oracle_behavior`, `candidate_behavior`,
   rationale, user-visible effect, compatibility risk, regression test,
   witnesses) — see `docs/KNOWN_DIVERGENCES.md`, or
2. not yet discovered — in which case the residual-ledger discipline
   (docs/COURTS.md) applies when it surfaces.

Corrections are never silent.
