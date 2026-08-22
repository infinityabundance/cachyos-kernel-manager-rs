# config-roundtrip/canonicalization

Non-VM differential court for the config file surface (§21): the candidate's
`KernelManagerConfig` (toml 0.8) vs the oracle's `config-option-lib`
(toml 1.1, the upstream's actual dependency), over a frozen 10-file corpus:

| corpus file | exercises |
|---|---|
| `full.toml` | all 18 fields, as the oracle's save would produce |
| `minimal.toml` | omitted fields default on load, all fields present after re-serialize |
| `empty.toml` | all defaults |
| `unknown-fields.toml` | extra fields ignored (no `deny_unknown_fields`) |
| `invalid-enum-value.toml` | free-form strings tolerate invalid enum-like values |
| `unicode-name.toml` | literal UTF-8 in custom names |
| `quotes-name.toml` | escaped quotes in basic strings |
| `long-name.toml` | 500-char custom name preserved exactly |
| `malformed.toml` | both sides exit 1 with empty stdout |
| `crlf.toml` | CRLF line endings tolerated |

Witness: `tools/run-config-corpus.sh` runs both CLIs over the corpus and
writes `oracle/<name>.canonical` / `oracle/<name>.exit` and the candidate
equivalents; `cargo xtask court run config-roundtrip/canonicalization`
fingerprints and byte-compares them.

Status: defined. Run:

```
tools/run-config-corpus.sh
cargo xtask court run config-roundtrip/canonicalization
```

Falsifier: any byte difference in canonical output or any exit-code mismatch
on any corpus file.
