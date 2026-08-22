# scx/loader-interface

Non-VM differential court for the typed `org.scx.Loader` D-Bus interface:
the candidate's `loader_interface` descriptor (scx crate, rendered by
`cachyos-kernel-manager-scx-introspect`) vs the source-derived reference
re-declaring `scx_loader 1.0.9 src/dbus.rs` (the crate version pinned by
the frozen `config-option-lib/Cargo.lock` checksum, archived in
`oracle/scx-authority/`).

The interface (service `org.scx.Loader`, path `/org/scx/Loader`; D-Bus names
are the PascalCase forms zbus derives from the snake_case Rust names):

- methods: `StartScheduler(s,u)`, `StartSchedulerWithArgs(s,as)`,
  `StopScheduler()`, `SwitchScheduler(s,u)`,
  `SwitchSchedulerWithArgs(s,as)` — all `()`;
- read-only properties: `CurrentScheduler: s`,
  `SchedulerMode: u` (repr-less fieldless enum → u32),
  `SupportedSchedulers: as`.

The VM variant is the PRIMARY witness (the REAL running loader's wire
interface + property values) and has PASSED on the reference image's
scx_loader (`scx-manager 1.15.12-1`): the candidate's five methods and
three read properties are all present on the real loader with identical
signatures, and the readback matches (CurrentScheduler `unknown`,
SchedulerMode `0`, SupportedSchedulers the 13 shipped schedulers). The
shipped loader is a LATER version than the frozen 1.0.9 and exposes MORE
surface (RestartScheduler/RestoreDefault methods, CurrentSchedulerArgs /
DefaultMode / DefaultScheduler properties) — the frozen oracle never calls
those, and the candidate's client is exactly the frozen surface (a
faithful SUBSET, proven by the court).

There is no corpus: the interface is fixed by the type system on both
sides. `tools/run-scx-corpus.sh` renders it once per side (the
source-derived comparison); `cargo xtask court run scx/loader-interface
--vm` runs the full differential against the real loader.

Status: defined. Run:

```
tools/run-scx-corpus.sh
cargo xtask court run scx/loader-interface
cargo xtask court run scx/loader-interface --vm   # the real-loader witness
```

Falsifier: any candidate interface element absent or signature-mismatched
on the real loader; any readback value differing from the real loader's
property values.
