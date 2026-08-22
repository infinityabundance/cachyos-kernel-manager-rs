# build-env/env-rendering

Non-VM differential court for the build environment string (§22): the
candidate's `BuildOptions::env_string` vs the oracle's `get_all_set_values`
(`conf-window.cpp:421-451` + `compile_options.json` option_map), over a
frozen 10-case corpus of UI option states:

| corpus file | exercises |
|---|---|
| `default.json` | the Configure window defaults |
| `all-on.json` | every checkbox on, every combo non-default |
| `all-off.json` | every checkbox off, lto=none (no suffix workaround) |
| `combos-nondefault.json` | all six combos non-default |
| `cpu-manual-skip.json` | `_processor_opt` omitted at manual |
| `lto-none-no-suffix.json` | `_use_lto_suffix` absent at lto=none |
| `sentinel-custom.json` | custom name `$pkgbase` suppresses the suffix |
| `empty-custom.json` | empty custom name keeps the suffix |
| `thin-dist.json` | `_use_llvm_lto=thin-dist` |
| `mixed.json` | arbitrary mix incl. unicode custom name |

Witness: `tools/run-env-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.env` / `.exit` + candidate equivalents);
`cargo xtask court run build-env/env-rendering` byte-compares them.

Status: defined. Run:

```
tools/run-env-corpus.sh
cargo xtask court run build-env/env-rendering
```

Falsifier: any byte difference in the env string or any exit-code mismatch
on any corpus file.
