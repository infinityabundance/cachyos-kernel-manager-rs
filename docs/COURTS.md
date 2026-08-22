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

## Phase 6 status (build subsystem) — SEALED

Phase 6 closed with all build-subsystem surfaces courted. The Phase 6
courts (12 new, all PASS with verified evidence):

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
- `patch-injection/source-array` + `custom-name/pkgbase-injection` — PASS
  (differential VM courts on fixture `build-mutation`). The REAL Configure
  window is driven through AT-SPI/XTEST (patch list + custom name + Build
  kernel); the mutated PKGBUILD bytes are compared against the candidate's
  mutation model (`cachyos-kernel-manager-mutate`), including the
  source-array probe (real bash, popen newline-strip) and the
  broken-pkgver path (D-004: the oracle runs with cwd =
  `/root/.cache/cachyos-km/pkgbuilds`).
- `build-env/env-rendering` — PASS (non-VM). The oracle's
  `get_all_set_values` (conf-window.cpp:421-451 + compile_options.json
  `option_map`) vs the candidate's `BuildOptions::env_string` over a
  10-case UI-state corpus. Caught a real bug: `lto` maps to
  `_use_llvm_lto` (RES-2026-012; the candidate initially emitted `_lto`).
- `build-env/lifecycle` — PASS (non-VM). The oracle's `on_execute` +
  `finished_proc` + `aur_kernel.cpp` decisions vs the candidate's
  `BuildFlowPlan` over a 6-case (variant, cwd, globs) corpus: the
  cpusched_path, the mutable-cwd working_path quirk (D-004), the repo
  build command (`-scf` + `&& touch .done-status`), the terminal-helper
  argv, the done-status path, the AUR command (`-sicf`, gap-006), and the
  artifact-install command (`sudo pacman -U`).
- `build-env/failure-lifecycle` — PASS (non-VM). `finished_proc`
  (conf-window.cpp:378-405) vs the candidate model over a 7-case corpus:
  success keys on the `.done-status` FILE (not the exit code), the
  stdout lines (`success` / `pressed yes` / `pacman_cmd := ...`), the
  failure stderr line (`process failed with exit code: <n>`), and the
  re-entrant install quirk — the install's OWN completion prints
  `process failed with exit code: 0` even on pacman success.
- `build-env/cancellation` — PASS (non-VM). The `m_running` guard
  (a second OK click while running is a no-op) + the unconditional close
  (default `closeEvent`; window destruction terminates the in-flight
  `QProcess` child) vs the candidate `configure_trace` over a 6-case
  action-sequence corpus.
- `option-transitions/variant-switch` — PASS (non-VM). The oracle's
  combo-switch transitions (conf-window.cpp:553-602) vs the candidate's
  `VariantSwitchState` over 6 transition sequences.
- `artifact-glob/package-functions` — PASS (non-VM). The oracle's glob
  pipeline (conf-window.cpp:218-298) vs the candidate's bash-probe model
  over 8 PKGBUILDs × 4 PKGEXT cases; a top-level `exit 1` triggers the
  broken-pkgver path.
- `aur/enablement-matrix` — PASS (non-VM). The oracle's AUR support
  (kernel.cpp:253-283 discovery + 288-304 commit + aur_kernel.cpp:32-55)
  vs the candidate's AUR model over a 7-case corpus: the
  `ENABLE_AUR_KERNELS` gate (the SHIPPED CMake oracle has AUR compiled
  out — the disabled side is witnessed with paru installed and AUR
  packages listed; the enabled side is witnessed against the
  source-derived reference), the `!paru || !awk` gate message (5b075dc
  flip, compared byte-exact on stderr), the `!kernels.empty()` probe
  gate, `-headers` stripping + dedup, the `aur/<name>` /
  `unknown-version` rows, and the AUR-first commit order
  (git-refresh `~/.cache/cachyos-km/aur_pkgbuilds/<name>` from
  `https://aur.archlinux.org/<name>.git`, then `makepkg -sicf
  --cleanbuild --skipchecksums`, then `pacman -S --needed`, then
  `pacman -Rsn`), including the `headers`-substring build skip.

Phase 6 build-subsystem models live in the build and exec crates
(unit-courted): `git_cache_plan` (prepare_git_repo: create-dirs, enter,
non-git-dir wipe + re-clone quirk, checkout --force master / clean -fd /
pull refresh chain, cwd mutation), `clean_env_plan`/`env_assignments`
(restore_clean_environment: unset previous, re-apply, truncation quirk at
second `=` boundary, D-005 skip of the oracle's out-of-bounds read),
`BuildFlowPlan`, `finished_proc`, `configure_trace` (exec crate), and the
AUR model (`discover_aur` / `expand_aur_install` / `commit_commands` with
`aur_enabled` gating, plan crate).

## Phase 7 status (SCX) — SEALED

The SCX authority was recovered: the external scxctl-ui library was
extracted FROM this repo at `cc79698`, and its final in-repo state
(`f3eeaf6`) plus the pinned `scx_loader` 1.0.9 crate (checksum = the
frozen `config-option-lib/Cargo.lock`) are archived in
`oracle/scx-authority/` (`cargo xtask scx verify` checks them). 8 courts
PASS (all with verified evidence):

- `scx/button-visibility` — the sched-ext button hide decision
  (`km-window.cpp:185-188`); the present direction is VM-witnessed by the
  kernel-discovery evidence (the button is `visible` in the a11y tree).
- `scx/current-scheduler` — the sysfs state/ops readback (`unknown` for
  enabled + empty ops, the state text otherwise).
- `scx/mode-flags` — the scx_loader per-(sched, mode) flag matrix + the
  config override/fallback.
- `scx/window-init` — the SchedExtWindow init sequence (config-init stop,
  no-loader stop + widget hiding, the population trace).
- `scx/profile` — the bpfland/lavd-only profile visibility + flags render.
- `scx/apply` — `apply_scheduler_change`: service disable, the
  args-vs-mode decision (b70b01b), the `Stoping scx service` typo, loader
  enable, the pkexec copy.
- `scx/disable` — `disable_scheduler`: stop_scheduler + pkexec copy,
  default_sched cleared.
- `scx/loader-interface` — the typed org.scx.Loader surface: non-VM
  source-derived comparison AND the VM real-loader witness: the candidate
  interface is a faithful SUBSET of the shipped loader's (scx-manager
  1.15.12-1), the readback values match (CurrentScheduler
  `unknown`, SchedulerMode `0`, the 13 supported schedulers).

The typed client (`crates/cachyos-kernel-manager-scx`, feature `dbus`)
uses zbus 5.5.0 / zvariant 5.4.0 — the frozen authority's exact versions.

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
