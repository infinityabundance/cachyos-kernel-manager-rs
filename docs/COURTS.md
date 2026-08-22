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

## Phase 5 status (execution / privilege)

10 more courts PASS (total 28), all with verified evidence:

- **Transaction courts** — the REAL oracle GUI is driven through AT-SPI
  (checkbox indicator click + Execute) under strace; the witnessed exec
  chains are compared witness-by-witness against the candidate plan tool's
  modeled chains (probe argv, pacman argv, terminal-helper argv):
  - `nvidia-companion/dkms-profile` — chwd nvidia-dkms → prebuilt nvidia
  - `nvidia-companion/open-profile` — chwd nvidia-open-dkms → prebuilt open
  - `nvidia-companion/dkms-installed` — nvidia-dkms installed → NO companion
  - `nvidia-companion/modules-installed` — module family reuse (pacman -Qqs)
  - `zfs-companion/root-on-zfs` — findmnt=zfs → zfs companion first
  - `kernel-removal/plan` — removal list kernel→headers→zfs→nvidia
  - `kernel-removal/update-available-execute` — the upgrade quirk: BOTH
    `pacman -S --needed` AND `pacman -Rsn` for the same kernel, in order
- `terminal-helper/emulator-matrix` — exit-code surface per emulator stub
  (none→1 with tmp-file LEAK, first-fails→2, kgx-fails→0, success→2 (!),
  `-s` override); the success→2 result is the `A || B && C` precedence
  quirk in the upstream script (only kgx failure avoids exit 2)
- `privilege/polkit-identity` + `privilege/helper-scripts` — byte-identity
  of the polkit policy and the two installed Bash helpers (packaging-level)

The residuals encountered in Phase 5 are in `atlas/residual-ledger.json`
(RES-2026-011: the candidate's installed-set was built from discovered
kernels only, so `nvidia-dkms` was invisible; fixed with full local-db
enumeration).

Drift/slew: `minimal` ×3 and `upgrade-available` ×2 re-runs on identical
overlays are deterministic.

## Phase 6 status (build subsystem, part 1)

- `config-roundtrip/canonicalization` — PASS (non-VM differential court). The
  candidate's `KernelManagerConfig` (toml 0.8) vs the oracle's
  `config-option-lib` struct verbatim (toml 1.1, the upstream's actual
  dependency, via `tools/config-oracle-ref`) over a frozen 10-file corpus
  (all-fields / minimal / empty / unknown-fields / invalid-enum-value /
  unicode / quotes / 500-char / malformed / CRLF). Canonical
  re-serialization is byte-identical and exit codes match (0 parse ok /
  1 parse error) on every corpus file. Witness:
  `tools/run-config-corpus.sh` → `cargo xtask court run
  config-roundtrip/canonicalization`.

Phase 6 build-subsystem models landed in the build crate (unit-courted):
`git_cache_plan` (prepare_git_repo: create-dirs, enter, non-git-dir wipe +
re-clone quirk, checkout --force master / clean -fd / pull refresh chain,
cwd mutation) and `clean_env_plan`/`env_assignments`
(restore_clean_environment: unset previous, re-apply, truncation quirk at
second `=` boundary, D-005 skip of the oracle's out-of-bounds read).

- `git-cache/lifecycle` — PASS (differential VM court on fixture
  `git-cache`). The REAL GUI Configure button is clicked through AT-SPI
  under strace (`oracle-configure.py` / `oracle-configure.sh`); the
  witnessed `prepare_git_repo` refresh chain — `git checkout --force
  master`, `git clean -fd`, `git pull` — is compared witness-by-witness
  against the candidate model (`cachyos-kernel-manager-gitcache` over
  `git_cache_plan`, schema `cachyos-km-candidate-plan-v1`). The fixture
  seeds `/root/.cache/cachyos-km/pkgbuilds` as a checkout of a local bare
  remote (offline, remote ahead by one commit so the refresh really
  fast-forwards).

Defined, awaiting bake + differential run: `patch-injection/*`,
`custom-name/*`, `build-env/lifecycle`, `artifact-glob/package-functions`.

The residuals encountered in Phase 5 are in `atlas/residual-ledger.json`
(RES-2026-011: the candidate's installed-set was built from discovered
kernels only, so `nvidia-dkms` was invisible; fixed with full local-db
enumeration).

The residuals encountered while establishing the baseline (and their
resolutions) are recorded in `atlas/residual-ledger.json`
(RES-2026-001..011).

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
