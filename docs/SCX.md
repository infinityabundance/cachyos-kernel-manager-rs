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

## Authority (Phase 7)

The frozen binary's SCX window lives in the external scxctl-ui library,
but that library was extracted FROM this repository at `cc79698`; its
final in-repo state (the parent `f3eeaf6`) plus the pinned `scx_loader`
crate (1.0.9, checksum = the frozen `config-option-lib/Cargo.lock`) are
the recoverable SCX authority:

- `oracle/scx-authority/scx-manager-f3eeaf6.tar.gz` — the pre-extraction
  scx-manager (the SchedExtWindow: init sequence, profiles, bpfland/lavd
  restriction, apply/disable) + the Rust D-Bus/config layer
  (`config-option-lib/src/scx_loader_config.rs`);
- `oracle/scx-authority/scx_loader-1.0.9.crate` — the org.scx.Loader wire
  interface (`src/dbus.rs`, `src/main.rs`, `src/config.rs`);
- `oracle/UPSTREAM.lock [scx]` records both archives' hashes;
  `cargo xtask scx verify` checks them.

## Candidate architecture (Phase 7 — SEALED)

```text
Slint UI → typed Rust SCX client → D-Bus (org.scx.Loader) → scx_loader
```

`crates/cachyos-kernel-manager-scx`:

- pure decision models (no D-Bus needed): `config` (SupportedSched /
  SchedMode / the default per-mode flag matrix / scx_loader.toml),
  `interface` (the typed org.scx.Loader surface as an inspectable
  descriptor — the single source the zbus proxy and the introspect
  witness are generated from), `state` (the sysfs current-scheduler
  readback), `apply` (the apply/disable decision traces), `window` (the
  button visibility + SchedExtWindow init/profile/apply UI decisions);
- the typed client (`client`, feature `dbus`): a zbus 5.5.0 / zvariant
  5.4.0 proxy — the EXACT versions the frozen authority pins, so the wire
  encoding matches scx_loader 1.0.9 byte-for-byte. The D-Bus names are
  the PascalCase forms zbus derives (`StartScheduler`, `CurrentScheduler`,
  ...); `SchedulerMode` is `u` (a repr-less fieldless enum → u32).

## Courts (Phase 7 — all PASS)

- `scx/button-visibility` — the main-window hide decision (`km-window.cpp:185-188`);
  the file-present direction is additionally VM-witnessed by the
  kernel-discovery evidence (the button is `visible` in the a11y tree).
- `scx/current-scheduler` — the sysfs state/ops readback branching.
- `scx/mode-flags` — the per-(sched, mode) flag matrix + config
  override/fallback.
- `scx/window-init` — the SchedExtWindow init sequence (the two stop
  paths, the population trace).
- `scx/profile` — the bpfland/lavd-only profile visibility + flags render.
- `scx/apply` — the apply_scheduler_change trace (service disable, the
  args-vs-mode decision, loader enable, pkexec copy).
- `scx/disable` — the disable trace + the config mutation.
- `scx/loader-interface` — the typed surface: non-VM source-derived
  comparison AND the VM real-loader witness: `cargo xtask court run
  scx/loader-interface --vm` starts the REAL scx_loader (scx-manager
  1.15.12-1 in the reference image) on the system bus, `busctl
  introspect`s it, and proves the candidate's interface is a faithful
  SUBSET (the shipped loader exposes more — RestartScheduler,
  RestoreDefault, CurrentSchedulerArgs/DefaultMode/DefaultScheduler —
  which the frozen oracle never calls) and that the candidate's typed
  client reads the same property values.

All Phase 7 surfaces are courted; nothing further is pending here.
