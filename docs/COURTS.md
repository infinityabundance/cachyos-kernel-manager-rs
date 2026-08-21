# COURTS

FRF parity courts. A court is a reproducible case directory proving one
compatibility claim through differential execution against the oracle.

## Evidentiary chain

```
CLAIM → MODEL → ASSUMPTIONS → OBSERVABLES → WITNESS → INDEPENDENCE → FALSIFIER → EVIDENCE
```

## Case format

```
courts/<domain>/<case>/
  claim.toml          claim + model pointer + falsifier
  assumptions.toml    environmental assumptions
  fixture/            static inputs (package DB dumps, PKGBUILDs, configs, ...)
  oracle/             raw observations from the oracle run
  candidate/          raw observations from the candidate run
  comparator.toml     normalizer pipeline + comparison rules
  residual.json       machine-read residuals (empty when parity holds)
  evidence.json       hashes + receipts
  README.md           human explanation
```

Raw evidence is immutable; normalization is explicit and versioned
(`name/version/source_hash/input_domain/transformation/justification/
falsifier/tests` per normalizer). Raw evidence is never overwritten.

## Residual discipline

When a residual appears, do NOT immediately patch the candidate. Determine:

- What differed? At which layer?
- Was the fixture really identical?
- Was an observable normalized?
- Is the oracle nondeterministic?
- Is this historical behavior?
- Is there hidden external state?
- Is the source misleading?
- Is the model incomplete?

Record it in the residual ledger (`atlas/residual-ledger.json`) with:
`id, court, first_observed, oracle_fingerprint, candidate_fingerprint,
classification, root_cause, resolution, commit, regression_witness`.
No unexplained residual disappears from history.

## Planned court domains (from the oracle atlas)

| domain | key cases |
|---|---|
| `kernel-discovery` | ordinary, cachyos variants, lts, custom repo, api-headers exclusion, kernel-without-headers, headers-without-kernel, duplicate names across dbs, installed-from-other-repo, stale db |
| `version-state` | upgrade/downgrade/equal, epoch, pkgrel, unusual version syntax, marker rendering, vercmp sort |
| `category` | classifier substring matrix incl. precedence |
| `nvidia-companion` | full decision matrix + chwd outputs incl. malformed |
| `zfs-companion` | root-zfs/non-root/zfs-absent/findmnt failure |
| `transaction-plan` | selection → plan expansion (order, reasons, dedup) |
| `pacman-config` | mINI vs pacman parsing: repos, testing, options, include, malformed, empty, ordering |
| `terminal-helper` | emulator matrix, exit codes, precedence quirk, no-terminal |
| `privilege` | rootshell argv chain, polkit action identity, env minimization |
| `patch-injection` | PKGBUILD mutation residuals: none/single/multi/local/remote/mixed/spaces/unicode/metachars/multiline |
| `custom-name` | pkgbase injection incl. hostile inputs |
| `build-env` | env var rendering, `_use_lto_suffix`, leakage between builds |
| `option-transitions` | variant switch matrix (cachyos↔lts↔hardened↔bore↔rt↔rc↔server↔...) |
| `config-roundtrip` | load→save semantic + serialized residual parity, outdated values, malformed, unknown fields |
| `artifact-glob` | package_* functions, split packages, PKGEXT variants, stale artifacts |
| `git-cache` | prepare_git_repo lifecycle incl. non-git dir removal, cwd mutation |
| `single-instance` | lock identity, stale lock, exit -1 |
| `i18n` | locale resolution incl. uk-not-in-qrc, partial catalogs |
| `error-paths` | injected ENOENT/EACCES/nonzero/signal/timeout per external dependency |
| `scx` | org.scx.Loader states (Phase 7) |

## Host safety

Destructive or privileged courts run only inside disposable VMs. The harness
fails closed unless the machine presents: expected machine-id class, fixture
marker, snapshot identity, and a test root. Implemented in code
(`crates/cachyos-kernel-manager-casefile` + xtask gate), not just documented.

## Ledger & coverage

- `atlas/coverage-gaps.json` — known un-courted surfaces (unknown-surface
  hunt output).
- Court coverage ledger: every surface in `atlas/inventory.json` maps to ≥1
  court or an explicit rationale; tracked in `atlas/court-ledger.json`.
- Release gate: required courts pass, 0 unexplained residuals, 0 flaky
  courts, all divergences evidence-backed.
