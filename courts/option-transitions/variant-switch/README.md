# option-transitions/variant-switch

Non-VM differential court for the Configure window's dynamic option
transitions (§20): the candidate's stateful `VariantSwitchState` model vs
the oracle's `main_combo_box` change handler (`conf-window.cpp:553-602`),
over a frozen 6-sequence corpus:

| corpus file | exercises |
|---|---|
| `all-variants.json` | every variant once, in combo order |
| `round-trip-thin-dist.json` | lts/cachyos/hardened round trips (count 3<->4 add/remove) |
| `server-defaults.json` | server lazy/300 defaults + cachy_config |
| `rt-zfs.json` | rt zfs disable + force-uncheck, never re-checked |
| `preempt-extension.json` | Voluntary/None add/remove round trips |
| `adjacent.json` | a second full pass in a different order |

Witness: `tools/run-option-corpus.sh` (runs both CLIs over the corpus and
writes `oracle/<name>.json` / `.exit` + candidate equivalents);
`cargo xtask court run option-transitions/variant-switch` byte-compares them.

Status: defined. Run:

```
tools/run-option-corpus.sh
cargo xtask court run option-transitions/variant-switch
```

Falsifier: any difference in the control state after any switch of any
sequence.
