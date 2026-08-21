# ORACLE — the frozen authority

The behavioral oracle is the real upstream CachyOS Kernel Manager at a frozen
revision, executed inside disposable VMs. Source archaeology explains behavior;
differential execution proves it.

## Freeze record (Phase 0)

See [`oracle/UPSTREAM.lock`](../oracle/UPSTREAM.lock) — the immutable authority file.

| Field | Value |
|---|---|
| repository | `https://github.com/CachyOS/kernel-manager` |
| branch | `develop` (only branch) |
| commit | `6b4a373e6a4e7295a0803034e597c4f2a055a411` |
| tree | `8c07748004ff255368649f6263b48556ff4f04de` |
| version | `1.19.0` |
| tag | `v1.19.0` (peeled commit == develop HEAD at freeze) |
| retrieved | 2026-08-21 |
| source archive | `oracle/upstream-v1.19.0-6b4a373.tar.gz` |
| archive sha256 | `1e464db65e410e4452e47ae619bbc490f976e7fe62cfe4936bf3fd96b0680e8f` (deterministic: `git archive`, verified reproducible) |

The frozen checkout lives at `oracle/upstream` (a clone at the pinned commit,
kept only for archaeology; the tarball is the immutable record).

## Authority hierarchy

When evidence conflicts:

1. real current upstream behavior under controlled execution (VM oracle)
2. current upstream source (frozen here)
3. current packaging scripts and installed package contents
4. upstream tests
5. upstream commit history
6. issue/PR discussions and maintainer comments
7. documentation
8. assumptions

Documentation (README, comments) is useful evidence but never an oracle.
The README's claims are contradicted by source in places — e.g.
`terminal-helper`'s header comment claims `$TERMINAL` must come first, but the
code never reads it.

## Upstream drift

Upstream updates are handled as **new oracle revisions**, never by silently
moving the pinned commit. Each revision produces:

- a new `oracle/UPSTREAM.lock`
- a new source archive + hash
- a residual analysis against the previous revision (what changed, which
  courts are impacted, what must be re-run)

Tooling: `cargo xtask upstream diff` (compares locked revision vs a candidate
ref and lists changed surfaces, commands, strings, paths, options, policy).

## VM oracle (Phase 2, active)

Docker is the orchestrator, not the machine model. Disposable QEMU/KVM VMs
carry the oracle application and the candidate; snapshots are restored between
the two runs so both observe identical machine state.

Safety invariant (implemented in code, not just documented): destructive or
privileged courts fail closed unless they can prove they are inside an
approved disposable VM (machine-id class + fixture marker + snapshot identity
+ test root). The developer host package database is never a mutation target.

## Package hashes (Phase 2, active)

`package_hashes` in the lock file is populated (`cargo xtask oracle pkg-hash`)
and `reference_image_hash` is populated (`cargo xtask vm build`). The base
image was rebuilt on 2026-08-21 after the cachyos pacman.conf restoration
(the pacman package's own /etc/pacman.conf overwrote the CachyOS config
during pacstrap, hiding linux-cachyos from oracle discovery); the new hash is
`sha256:db67e678…` in `oracle/UPSTREAM.lock`. Future base rebuilds will
produce new hashes — treat each as a new reference-image revision with its
own residual analysis (the package manifest in `vm/images/manifest.json`
pins exact package versions).
