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
| D-003 | custom pkgbase / patch splice validation | SECURITY_CORRECTION → IMPLEMENTED 2026-08-23 (see the D-003 ledger entry below) — the splice boundary rejects quote/newline/$/backtick/backslash bytes |
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

## D-008 — Configure close terminates the in-flight build (IMPLEMENTED, availability correction)

- **oracle_behavior**: the frozen app owns ONE persistent `ConfWindow` in a
  `unique_ptr`; `on_cancel` calls `close()`, and `closeEvent` just delegates
  to `QWidget::closeEvent`. `WA_DeleteOnClose` is never set, so `close()`
  only HIDES the window — the `QProcess m_cmd` member is NOT destroyed and
  an in-flight build/install KEEPS RUNNING after the window hides
  (audit P0 correction 2026-08-23: the earlier model claiming the
  QProcess destructor terminated the child was wrong — framework
  object-lifetime semantics, not application source; `configure_trace`
  now models close as hide-with-build-continuing).
- **candidate_behavior**: the Configure window's Cancel/Close TERMINATES the
  in-flight build/install (an operation-generation token + the owned
  terminal-helper child are killed; the worker reports the failure branch).
- **reason_for_divergence**: INTENTIONAL_CORRECTION (availability): the
  oracle leaves the build running invisibly after the window hides — the
  user has no way to cancel it and no progress feedback. The candidate
  makes the hide an actual cancel so the user is never left with an
  invisible runaway makepkg/pacman.
- **user-visible_effect**: closing Configure mid-build aborts the build
  (and the artifact install, which is owned by the same mechanism); the
  oracle would keep building invisibly.
- **compatibility_risk**: a user who closes Configure expecting the build
  to continue (oracle behavior) now gets a cancelled build. Documented;
  the VM oracle court (Phase 12) witnesses the difference.

Related (same correction family, MAIN window): closing the app during a
TRANSACTION. The frozen oracle ABORTS — `closeEvent` (km-window.cpp:
327-338) releases the alpm handle and lets the app exit while the worker
QThread is still blocked in `runCmdTerminal`; Qt aborts with "QThread:
Destroyed while thread is still running" (SIGABRT, witnessed 2026-08-23
by `ui/close-during-transaction`, fixture `close-transaction` — the
gap-010 race-hunt mandate). The candidate exits CLEANLY (the transaction
task is a runtime-owned detached thread; `Effect::Close` exits the event
loop); the machine residuals match byte-for-byte (the close corrupts
nothing on either side — the in-flight transaction is left to the
terminal the same way).
- **safety_or_correctness_rationale**: no orphaned root/pacman/makepkg
  processes; deterministic failure instead of invisible progress.
- **regression_test**: `configure_trace` (close hides, build keeps
  running — the CORRECTED oracle model) + the UI cancellation tests
  (generation bump, pre/post-spawn abort checks, install ownership).
- **oracle_witness**: the frozen source's ConfWindow ownership (unique_ptr,
  no WA_DeleteOnClose) + `configure_trace`; the Phase 12 VM oracle court
  drives a real Qt Configure-close-during-build and records the build
  continuing.
- **candidate_witness**: `cancel_build_process` + the epoch checks in
  `build_task`/`artifacts_task`.
- **maintainer_notes**: if the oracle model is ever re-witnessed as
  destroying the process (a Qt version difference), re-evaluate; the
  VM court is the authority.

## D-003 — PKGBUILD splice validation (IMPLEMENTED, witnessed)
- **oracle_behavior**: the custom package name (`set_custom_name_in_pkgbuild`,
  conf-window.cpp:328-339) and the patch entries (`insert_new_source_array_
  into_pkgbuild`) are spliced into the PKGBUILD double-quoted bash text with
  NO validation — a value containing `"`, a newline, `$`, backticks, or a
  backslash escapes the splice and becomes PKGBUILD code that makepkg
  EVALUATES (arbitrary command execution in the user's build).
- **candidate_behavior**: the production build boundary (`build_task`)
  rejects any custom name or patch entry containing `"` / newline / CR /
  `$` / backtick / backslash / NUL (`splice_unsafe_index`, build crate); the
  build fails SAFELY (the failure branch) instead of injecting the value.
  Valid inputs splice byte-identically to the oracle (the courted
  mutate/patch-injection witnesses are unchanged).
- **reason_for_divergence**: SECURITY_CORRECTION — a hostile value must not
  become PKGBUILD code.
- **user-visible_effect**: an unsafe custom name or patch entry now fails
  the build with a stderr diagnostic instead of producing a broken/hostile
  PKGBUILD (which would have failed or worse). Valid inputs behave
  identically.
- **compatibility_risk**: none for valid inputs; a value the oracle would
  have spliced dangerously now fails fast.
- **safety_or_correctness_rationale**: the splice boundary is the correct
  place to enforce the invariant (the PKGBUILD is executed by makepkg).
- **regression_test**: build crate `splice_unsafe_index_rejects_splice_
  breaking_bytes`; the gate itself is in the production `build_task` (the
  UI soft-lock + splice regressions are covered by the rendering suite).
- **oracle_witness**: the frozen source's unvalidated splice
  (conf-window.cpp:328-339, 401-406).
- **candidate_witness**: `splice_unsafe_index` + the build_task gate.
- **maintainer_notes**: keep the validators in the build crate (the courted
  splice witnesses stay pure); the gate belongs to the production boundary.

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

## D-009 — shipped-binary licensing + MSRV after the Slint port (DECISION RECORD)
- **oracle_behavior**: the frozen package is GPL-2.0-or-later (Qt app), built
  with the project's historical toolchain; the frozen package declares
  `license=('GPL-2.0-or-later')`.
- **candidate_behavior**: the code remains GPL-2.0-or-later, but the SHIPPED
  GUI binary links Slint 1.17.1, which is offered under GPL-3.0-or-later (or
  Slint's commercial terms). GPL-2.0-or-later code may be combined with
  GPL-3.0-or-later code (2.0-or-later is GPLv3-compatible), so the combined
  distributed binary is licensed GPL-3.0-or-later; the packaging declares
  both licenses. MSRV: the workspace `rust-version = "1.85"` holds ONLY for
  the feature-minimal semantic workspace (default features — every semantic
  crate + the CI msrv job); the GUI feature pulls Slint 1.17.1, which itself
  declares `rust-version = "1.92"`, so the shipped `gui-alpm` binary
  requires Rust 1.92 (CI builds the GUI on stable).
- **reason_for_divergence**: LICENSING/DECISION — the Slint dependency's
  license terms constrain the combined binary; the old blanket
  `GPL-2.0-or-later` claim no longer described the distributed artifact. The
  MSRV statement was similarly stale (the 1.85 CI job never built the slint
  stack, which needs 1.92).
- **user-visible_effect**: package metadata now lists GPL-3.0-or-later
  (alongside GPL-2.0-or-later) for the combined binary; the documented MSRV
  is per-artifact (1.85 semantic workspace / 1.92 shipped GUI).
- **compatibility_risk**: none functional; a downstream repackager must
  honor Slint's GPL-3.0-or-later (or commercial) terms for the binary.
- **safety_or_correctness_rationale**: accurate licensing/version metadata
  is a release-blocking correctness item, not documentation polish.
- **regression_test**: the CI msrv job asserts 1.85 on default features
  only; the GUI job runs on stable. A toolchain bump on either side moves
  the CI jobs.
- **oracle_witness**: the frozen package's GPL-2.0-or-later declaration.
- **candidate_witness**: packaging/PKGBUILD (both licenses) + Cargo.toml
  rust-version comment + docs/RELEASE.md §CI.
- **maintainer_notes**: if the project ever buys a Slint commercial license
  or Slint changes its GPL option, update the package license + this record.

## D-010 — build-option environment applied per-child, never process-global (IMPLEMENTED, safety correction)
- **oracle_behavior**: `restore_clean_environment` (`utils.cpp:204-227`)
  mutates the PROCESS environment with `setenv`/`unsetenv` from a worker
  thread, then the makepkg child inherits it.
- **candidate_behavior**: the build-option assigns are carried in the probe
  script text (the oracle's own mechanism — the env string is spliced
  verbatim into the `get_source_array_from_pkgbuild` testscript,
  conf-window.cpp:204-216) and applied to the terminal-helper child via
  `Command::envs` (`spawn_cmd_terminal`'s `env` parameter). The manager's
  own process environment is NEVER mutated.
- **reason_for_divergence**: SAFETY_CORRECTION (audit P1/security):
  `std::env::set_var`/`remove_var` from a background worker while the Slint
  event loop, D-Bus and native-library threads run is documented-unsound on
  multithreaded programs; the oracle's setenv/unsetenv has the same defect.
  The variable-selection SEMANTICS are preserved (each child receives the
  exact assigns, unset-then-set ordering is implicit — a fresh child never
  carries a previous run's vars).
- **user-visible_effect**: none — makepkg sees the same build options
  (they reach it via the terminal emulator + shell inheriting the helper's
  env); the process env of the manager is clean.
- **compatibility_risk**: none for the build flow; a hypothetical caller
  that relied on the manager's OWN environment being mutated by a build no
  longer sees that side effect (which was the defect).
- **safety_or_correctness_rationale**: no cross-thread environment mutation;
  per-child env is race-free and the only sound approach in Rust.
- **regression_test**: the exec `spawn_cmd_terminal` env parameter + the
  build-env courts (the env string rendering is unchanged; the courted
  probe script embeds it).
- **oracle_witness**: the frozen `restore_clean_environment` setenv/unsetenv
  (`utils.cpp:204-227`).
- **candidate_witness**: `spawn_cmd_terminal(cmd, escalate, cwd, env)` +
  the removed `restore_clean_environment`/`PREVIOUSLY_SET_OPTIONS` in app.rs.
- **maintainer_notes**: the probe scripts already embedded the env (the
  oracle's design), so only the terminal-helper path needed the per-child
  carrier.
