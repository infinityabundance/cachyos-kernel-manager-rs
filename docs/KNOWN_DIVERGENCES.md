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
