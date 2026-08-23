# ui/configure-window-semantics

Non-VM differential court for the candidate's Configure-window semantic
model (`crates/cachyos-kernel-manager-ui/src/configure_window.rs`, rendered
by the `cachyos-kernel-manager-confwindow` bin) vs an independent
re-declaration of the frozen oracle's `conf-window.cpp` semantics
(`tools/confwindow-oracle-ref`), over the shared corpus
(`cachyos-km-configure-window-corpus-v1`).

The court pins, for every corpus scenario (default, variant switches to
hardened/rt/server/lts, switch sequences, patch reset/ops, custom-name
sentinel, save-state):

- the ctor defaults (`conf-window.cpp:475-546`): variant labels, hardly +
  cachy_config checked, the combo lists, LTO thin initially selected;
- the variant-switch handler (`conf-window.cpp:553-602`): thin-dist
  availability, lto/preempt/hz defaults, cachy_config, zfs, and the
  trailing `reset_patches_data_tab`;
- `reset_patches_data_tab` + the patch-list operations (the `.patch`
  filter, `file://` prefix, remote append, remove/move-up/move-down);
- the `_use_lto_suffix=n` workaround condition (`conf-window.cpp:446`);
- `on_save`'s UI-state feed (`conf-window.cpp:743-755`).

The variant-switch transitions themselves are the core
`VariantSwitchState`'s (courted by `option-transitions/variant-switch`);
this court pins the **window assembly** — the semantics the Slint Configure
window will be built on (Phase 8).

Status: defined. Run:

```
tools/run-confwindow-corpus.sh
cargo xtask court run ui/configure-window-semantics
```

Falsifier: any byte difference in any model field over any corpus file.
