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
