# RELEASE

Release gate and evidence requirements. **No release has been cut; this file
defines the gate.**

## Gate (directive §69)

A release candidate must ship an evidence summary:

```text
required courts: N
passed: N
known intentional divergences: M
unexplained residuals: 0
flaky courts: 0
security-critical unresolved findings: 0
```

Every intentional divergence links to its evidence
(`docs/KNOWN_DIVERGENCES.md` + court witnesses).

## Deliverables per release (directive §89)

- Implementation (native Rust + Iced)
- Compatibility atlas (`atlas/inventory.json` + ledger)
- Historical atlas (`docs/HISTORICAL_LORE.md`)
- Frozen oracle (`oracle/UPSTREAM.lock` + deterministic archive)
- VM farm (Phase 2)
- FRF courts (passing)
- Residual ledger (empty of unexplained entries)
- Security dossier (`docs/SECURITY.md`, `docs/PRIVILEGE_MODEL.md`)
- Drop-in packaging (`packaging/`, Phase 10)
- Content-addressed evidence bundle
- Custodian documentation (this `docs/` set)

## Process

1. `cargo xtask oracle verify` — freeze integrity.
2. Full court matrix (`cargo xtask court run --all` + VM courts).
3. Drift/slew runs on representative courts.
4. Hostile review checklist (directive §83) + fuzzing (Phase 12).
5. Evidence pack + signed summary.

## Evidence publication layer

Two concepts, deliberately distinct (the FRF evidence protocol):

- **court recipe** — the committed, reproducible instructions: the
  `courts/<domain>/<case>/` claim/assumptions/comparator/README + frozen
  `fixture/` inputs. *How to reproduce evidence.*
- **evidence release** — an immutable, content-addressed record of an
  ACTUAL execution: `evidence/releases/<name>/MANIFEST.json` (metadata +
  root hash), `COURTS.json` (per-court receipts: recipe hashes, artifact
  hashes, normalizer/comparator versions, fixture + base-image digests,
  locator), `RECEIPTS.json` (compact FRF receipts), `ROOT-HASH` (sha256 of
  COURTS.json). *The immutable execution that produced the claim.*

The raw (often huge) oracle/candidate artifacts are gitignored and
regenerable from the recipe; the release records their hashes so any future
archive (GitHub Release asset, Zenodo, OCI artifact) is verifiable against
this record. The release files themselves are small and committed.

Commands:

```sh
cargo xtask evidence release <name>     # assemble + write the release
cargo xtask evidence verify-release <name>  # verify hashes + root hash
cargo xtask evidence verify             # per-court evidence.json integrity
python3 tools/validate-atlas.py         # ledgers + courts + releases schema
```

## CI

- `.github/workflows/ci.yml` — the pure gate on every PR/commit: fmt,
  clippy `-D warnings`, `cargo test --workspace`, oracle lock verification,
  atlas/court/release schema validation, status-table derivation check,
  evidence verification, MSRV (1.85) check.
- `.github/workflows/forensic.yml` — the heavy differential matrix (VM
  build, fixture baking, `court run --vm` on every VM court, evidence
  release) on a KVM+docker self-hosted runner, manual/nightly only.
