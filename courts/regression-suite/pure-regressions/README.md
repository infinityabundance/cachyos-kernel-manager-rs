# regression-suite/pure-regressions

Historical-regression court (Phase 9): the RES-2026-002/003/004/012
resolutions re-verified live. The **oracle/** side is the declared
expectation (from `atlas/residual-ledger.json` + the frozen source), the
**candidate/** side is the current witness execution; any byte difference
is a regression.

Assertions:

- `env-lto.txt` — RES-2026-012: the env rendering emits `_use_llvm_lto=thin`
  (the oracle's `option_map` name; the candidate's former `_lto` was the
  bug);
- `cross-repo-row.txt` — RES-2026-004: the cross-repo-installed row is
  present, unchecked, NOT immutable (`km-window.cpp:97-104`);
- `abi-probe.txt` — RES-2026-002/003: the libalpm ABI witness (the
  `alpm_list_t` 3-pointer layout + the `installed_db` string return), or
  the `skipped (no libalpm)` note where libalpm is unavailable.

The full regression witnesses remain their own courts
(`alpm-ffi/abi-surface`, `ui/main-window-semantics`,
`build-env/env-rendering`); this court pins the specific regressions in
one re-runnable place (wired into CI).

Status: defined. Run:

```
tools/run-regression-corpus.sh
cargo xtask court run regression-suite/pure-regressions
```

Falsifier: any byte difference in any assertion file.
