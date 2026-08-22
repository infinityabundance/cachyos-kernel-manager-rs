# COURTS

FRF parity courts. A court is a reproducible case directory proving one
compatibility claim through differential execution against the oracle.

## Evidentiary chain

```
CLAIM → MODEL → ASSUMPTIONS → OBSERVABLES → WITNESS → INDEPENDENCE → FALSIFIER → EVIDENCE
```

## Execution model (Phase 2)

Two modes:

1. **Pure courts** (`cargo xtask court run <case>`): fingerprint the case's
   `oracle/` and `candidate/` directories; used for static corpora and any
   comparison that does not need the oracle binary.
2. **Differential VM courts** (`cargo xtask court run <case> --vm`): the
   real oracle GUI and the candidate inspect tool run against byte-identical
   fixture snapshots in disposable KVM VMs:

```
fixture image (baked OFFLINE in a chroot — loop-free: the exported base
  rootfs directory is copied, mutated by the spec, and mkfs.ext4 -d'd into
  a raw image; qcow2 digest recorded)
  → fresh overlay → boot → oracle (Xvfb + AT-SPI + strace) → oracle/ evidence
  → fresh overlay (restore identical snapshot) → boot → candidate → candidate/ evidence
  → FRF comparator (normalizers + field diff) → residual.json + evidence.json
```

The machine residual (pacman -Q, sync db hashes, local db) is captured on
both sides and compared first: any drift is a fixture-integrity violation,
not a parity pass.

Oracle observation is AT-SPI based (directive §37 prefers accessibility-tree
over coordinate screenshots; §0 forbids screenshots as proof). The probe
commands the oracle executes at startup (findmnt/chwd/pacman -Qqs/...) are
witnessed via strace `execve` capture for archaeology.

## Baseline status

18 courts PASS (oracle == candidate, 0 residuals, evidence verified): the 10
Phase 2 kernel-discovery courts (`minimal`, `several-kernels`,
`upgrade-available`, `downgrade-visible`, `custom-repo`,
`cross-repo-installed`, `duplicate-across-repos`, `stale-db`,
`empty-all-dbs`, `empty-sync-db`), and the 8 Phase 4 courts
(`kernel-discovery/adversarial-names`, `epoch-versions`,
`companion-resolution`; `pacman-config/testing-and-disabled`,
`case-sensitivity`, `duplicated-sections`, `malformed`, `missing-conf`).

Drift/slew: `minimal` ×3 and `upgrade-available` ×2 re-runs on identical
overlays are deterministic.

The residuals encountered while establishing the baseline (and their
resolutions) are recorded in `atlas/residual-ledger.json`
(RES-2026-001..010).

Before running courts, `vm-ctl.sh cleanup` removes stale qemu processes
(they survive process-tree kills because they run in their own systemd
scopes); the court runner calls it automatically before each side.

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
