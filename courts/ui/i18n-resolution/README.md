# ui/i18n-resolution

Non-VM differential court for the candidate's translation layer
(`crates/cachyos-kernel-manager-ui/src/i18n.rs`, rendered by the
`cachyos-kernel-manager-i18n` bin) vs an independent re-declaration that
parses the frozen `lang/*.ts` files **directly as XML**
(`tools/i18n-oracle-ref`, roxmltree), over the shared corpus
(`cachyos-km-i18n-corpus-v1`).

The court pins, for every corpus scenario (de_DE, zh_CN, zh-CN, ru_RU, C,
fr_FR):

- the **resolution**: `initTranslations` load order (`main.cpp:62-106`)
  against the qrc alias set — `de_DE` → `de`; `zh_CN` → **no catalog**
  (gap-009: the compiled alias is `zh-CN`, never `zh`/`zh_CN`); `zh-CN` →
  `zh-CN`; `fr_FR` → **no catalog** (French is not compiled);
- the **translations**: `QTranslator::translate(context, source)`
  semantics (first non-empty, finished `<translation>`; else the source).

The candidate's data comes from the embedded ts2json catalogs (checked in,
CI-verified by `tools/ts2json.py --check`); the oracle side re-parses the
`.ts` directly — the court proves the pipeline's output equals the frozen
authority's.

Status: defined. Run:

```
tools/run-i18n-corpus.sh
cargo xtask court run ui/i18n-resolution
```

Falsifier: any byte difference in any resolved translation, the resolved
alias, or the exit code, over any corpus file.
