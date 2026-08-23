# KNOWN DIVERGENCES

Ledger of every deliberate divergence from the oracle. **Currently empty of
implemented divergences** — this file exists to make the discipline
explicit. Divergences are only entered here when the candidate code exists
and the court witnesses are recorded.

Format per divergence:

```text
id
oracle_behavior
candidate_behavior
reason_for_divergence
user-visible_effect
compatibility_risk
safety_or_correctness_rationale
regression_test
oracle_witness
candidate_witness
maintainer_notes
```

## Planned corrections (not yet implemented — nothing is claimed)

| id | area | nature |
|---|---|---|
| D-001 | `rootshell.sh` arbitrary root shell | SECURITY_CORRECTION → narrow typed helper + shim (docs/PRIVILEGE_MODEL.md) |
| D-002 | PKGBUILD/config non-atomic writes | INTENTIONAL_CORRECTION → atomic replace (crash resilience) |
| D-003 | custom pkgbase / patch splice validation | SECURITY_CORRECTION → reject quote/newline bytes that break the splice |
| D-004 | process-cwd mutation by git prep | INTENTIONAL_CORRECTION → derive build path from explicit cache path, not mutable cwd (user-visible parity: build dir stays `~/.cache/cachyos-km/pkgbuilds/linux-cachyos/<variant>`) |
| D-005 | `fix_path`/`restore_clean_environment` UB on empty/malformed input | INTENTIONAL_CORRECTION → defined behavior (no observable difference for valid inputs) |
| D-006 | desktop `Categories=Qt` | packaging-artifact reclassification, evidence-gated |

None of the above is entered into the formal ledger until implemented and
witnessed.

## Oracle quirks the candidate must preserve (not divergences)

See docs/HISTORICAL_LORE.md §"Known quirks inventory" — terminal-helper exit
code, uk-not-in-qrc, mINI pacman.conf parsing, `$TERMINAL` ignored,
version-marker glyphs, `Waiting... ` stderr spam, etc.

The oracle binary has NO `--version` handling: launching it with
`--version` aborts (Qt abort without a display — witnessed by
packaging/upgrade, baseline/reverted `--version` core dumps). The candidate
ADDS a `--version` flag (prints `cachyos-kernel-manager 0.1.0`) — an
additive CLI convenience, no user-visible effect on the GUI drop-in
surface (the oracle's abort is not a contract).

## D-007 — desktop entry StartupWMClass (IMPLEMENTED, witnessed)

- **oracle_behavior**: the installed desktop entry has NO StartupWMClass
  key; Qt's WM_CLASS (set by QApplication from the argv[0] basename) gives
  KWin the identity it needs to group the three windows under one taskbar
  entry.
- **candidate_behavior**: the desktop entry adds the 3-line explanatory
  comment + `StartupWMClass=org.cachyos.KernelManager`. The winit windows do
  not set a Qt-style WM_CLASS; the xdg app id (`set_xdg_app_id`) is the
  window's res_class, and StartupWMClass matches that so KWin groups the
  main/configure/sched-ext windows under the single taskbar icon (and the
  titlebar uses the correct green icon).
- **reason_for_divergence**: the toolkit switch (Qt → Slint/winit) removed
  Qt's automatic WM_CLASS; without the key the app shows as four separate
  taskbar entries and the wrong (yellow wayland) titlebar icon.
- **user-visible_effect**: the taskbar grouping + titlebar icon match the
  oracle's behavior (one entry, the cachyos icon) — the adaptation RESTORES
  parity the toolkit switch would otherwise break.
- **compatibility_risk**: none — StartupWMClass is a standard, ignored key
  for WMs that do not use it; the file-surface contract is unchanged except
  for the documented lines.
- **safety_or_correctness_rationale**: presentation-only; no effect on
  commands, paths, or state.
- **regression_test**: packaging/file-layout — the raw hash honestly
  differs on the desktop line; the normalized hashes
  (desktop-startupwmclass-strip v1) are byte-equal, proving the ONLY
  difference is the documented lines. Behavioral witness: the Phase 12
  taskbar/application-identity court (AT-SPI: one application entry, the
  correct app id, all three windows under it).
- **oracle_witness**: oracle/packages/cachyos-kernel-manager-1.19.0-1
  (frozen) desktop entry — no StartupWMClass.
- **candidate_witness**: packaging/usr/share/applications/org.cachyos.KernelManager.desktop
  (the StartupWMClass + comment) + the normalized file-layout hashes.
- **maintainer_notes**: if winit/wslay ever sets a Qt-compatible WM_CLASS
  natively, re-evaluate; the normalizer + court must move with the desktop
  file.
