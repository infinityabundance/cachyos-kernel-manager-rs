# Courts

Reproducible FRF parity courts. Layout per court (directive §44):

```
courts/<domain>/<case>/
  claim.toml          evidentiary chain (claim → model → assumptions →
                      observables → witness → independence → falsifier →
                      evidence)
  assumptions.toml    environmental assumptions + declared normalizers
  comparator.toml     comparison rules (ignore / byte_exact / json_semantic /
                      volatile prefixes)
  fixture/            immutable inputs (frozen before any run)
  oracle/             raw observations from the oracle run (VM)
  candidate/          raw observations from the candidate run (VM)
  residual.json       machine-readable residuals (written by xtask on failure)
  evidence.json       content-addressed receipts (produced by the runner)
  README.md           human explanation
```

Rules:

- Raw evidence is immutable; normalizers are explicit and versioned; raw
  evidence is never overwritten with normalized output.
- A residual is evidence, not an inconvenience: record it in
  `atlas/residual-ledger.json` before changing anything.
- Destructive/privileged courts run only inside disposable VMs and fail
  closed unless the machine proves it is an approved fixture.
- Unit tests in the crates are NOT courts. Courts are differential
  executions of the real oracle.

Tooling: `cargo xtask court list`, `cargo xtask court run <domain>/<case>`,
`cargo xtask court run --all`. VM orchestration lands in Phase 2.
