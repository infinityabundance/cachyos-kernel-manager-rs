# SCX

sched-ext integration status and facts.

## Oracle facts (revision `6b4a373e`)

- The main window embeds `scxctl::SchedExtWindow` from the external
  `scxctl-ui` library (`km-window.hpp:47,144`; CMake
  `find_package(scxctl-ui 1 REQUIRED)`).
- Button visibility: hidden unless `/sys/kernel/sched_ext/state` exists
  (`km-window.cpp:185-188`).
- History: scx-manager was extracted from this repository into its own
  library; D-Bus apply/disable logic moved into Rust (commits `425681d`,
  `c866d99`, `780b9b1`); args passed only when differing from defaults
  (`b70b01b`); running mode/scheduler shown as initial values (`c1e0525`);
  Server mode (`015a86d`); flags shown when changing profile/scheduler
  (`a147d57`).

## Candidate architecture (Phase 7)

```text
Iced UI → typed Rust SCX client → D-Bus (org.scx.Loader) → scx_loader
```

`crates/cachyos-kernel-manager-scx` currently records the interface facts and
the `SchedulerConfiguration` model. The D-Bus client, the visibility court,
and the state courts are Phase 7. Nothing here is claimed complete.
