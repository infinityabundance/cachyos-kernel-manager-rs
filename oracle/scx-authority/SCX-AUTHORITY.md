# SCX authority — the sched-ext surface of the frozen oracle

The frozen oracle (cachyos-kernel-manager v1.19.0, `6b4a373e`) embeds the
external `scxctl-ui` library (`km-window.hpp:47,144` —
`scxctl::SchedExtWindow`). That library was extracted FROM this repository
at upstream commit `cc79698` ("scx: move scx-manager into own repo"). Its
final in-repo state (the parent commit `f3eeaf6`) is therefore the closest
recoverable authority for the SCX window behavior, and the D-Bus layer's
crate dependency (`scx_loader` 1.0.9, pinned by the frozen
`config-option-lib/Cargo.lock` checksum) pins the `org.scx.Loader` wire
interface. Both are archived here, hash-verified.

## scx-manager (pre-extraction UI, `f3eeaf6`)

`scx-manager-f3eeaf6.tar.gz` — deterministic `git archive` of the frozen
repo at commit `f3eeaf6` (the parent of the extraction commit `cc79698`),
containing:

- `scx-manager/src/schedext-window-internal.cpp|hpp` — the SchedExtWindow:
  `get_current_scheduler` (sysfs `/sys/kernel/sched_ext/state` +
  `/sys/kernel/sched_ext/root/ops`), the 1s refresh timer, the init
  sequence (`init_config` / `get_supported_scheds` failure dialogs), the
  profile list (Auto/Gaming/Powersave/Lowlatency/Server), the
  bpfland/lavd-only profile visibility (`on_sched_changed`), the flags
  rendering (`on_sched_profile_changed`), `on_apply` / `on_disable`;
- `scx-manager/src/schedext-window.ui` — the window: window title
  "CachyOS Configure sched-ext", labels, `schedext_combo_box`,
  `schedext_profile_combo_box`, `schedext_flags_edit`, `current_sched_label`,
  Apply/Disable buttons;
- `scx-manager/src/scx_utils.{cpp,hpp}` — the cxxbridge Config wrapper;
- `config-option-lib/src/scx_loader_config.rs` — the Rust D-Bus + config
  layer the UI calls: `init_config_file`, `get_scx_flags_for_mode`,
  `apply_scheduler_change` (disable_scx_service, args-vs-mode decision,
  systemctl enable scx_loader, `/tmp/scx_loader.toml` + `pkexec cp`),
  `disable_scheduler` (default_sched=None + stop_scheduler + pkexec cp),
  `get_current_sched` / `get_current_mode` / `get_supported_scheds` via
  `scx_loader::dbus::LoaderClientProxy`;
- the `scxctl-ui` CMake config templates.

Recorded facts:

| fact | value |
|---|---|
| extraction parent commit | `f3eeaf6` |
| tree | `1f1829b2134fe83871ffb1682a3d294c05d603f9` |
| archive sha256 | `03086b2312e424d1ec90b5b1cfbe3211174c3938d5b07d8194119ae356a25260` |
| regeneration | `git -C oracle/upstream archive --format=tar.gz -o <out> f3eeaf6 scx-manager config-option-lib/src/scx_loader_config.rs cmake/scxctl-ui-config-version.cmake.in cmake/scxctl-ui-config.cmake.in` |

NOTE: the shipped binary links the LATER external scxctl-ui; the in-repo
state is the extraction-time behavior. The D-Bus interface (below) is
pinned independently by the crate version, so the wire contract does not
drift with the external UI.

## scx_loader 1.0.9 (the org.scx.Loader wire interface)

`scx_loader-1.0.9.crate` — the crates.io package, checksum-identical to the
frozen `config-option-lib/Cargo.lock`:

| fact | value |
|---|---|
| version | 1.0.9 |
| checksum (crates.io, matches the frozen Cargo.lock) | `1fb76102d4c18759eef97da4cb4a7450f968efcf711c006c0595f29f105429f0` |

Recovered wire interface (`scx_loader/src/dbus.rs` + `src/main.rs`, zbus 5.5.0
proxy/interface derives — D-Bus names are the PascalCase forms zbus derives
from the snake_case Rust names, `zbus_macros 5.5.0 utils.rs::pascal_case`):

- service `org.scx.Loader`, path `/org/scx/Loader`
- methods: `StartScheduler(s, u)`, `StartSchedulerWithArgs(s, as)`,
  `StopScheduler()`, `SwitchScheduler(s, u)`,
  `SwitchSchedulerWithArgs(s, as)`
- properties: `CurrentScheduler: s` (read), `SchedulerMode: u` (read),
  `SupportedSchedulers: as` (read)
- `SupportedSched` (`#[zvariant(signature = "s")]`): scx_bpfland,
  scx_rusty, scx_lavd, scx_flash; `SchedMode` (fieldless enum, explicit
  discriminants, no `repr` → zvariant u32): Auto=0, Gaming=1, PowerSave=2,
  LowLatency=3, Server=4
- default per-mode flags (`scx_loader/src/config.rs`):
  bpfland: Gaming `-m performance`, LowLatency `-s 5000 -S 500 -l 5000 -m
  performance`, PowerSave `-m powersave`, Server `-p`, Auto `[]`; lavd:
  Gaming/LowLatency `--performance`, PowerSave `--powersave`,
  Server/Auto `[]`; rusty and flash: `[]` for every mode
- config file: `default_sched: Option<SupportedSched>`, `default_mode:
  Option<SchedMode>`, `scheds: { name → {auto_mode, gaming_mode,
  lowlatency_mode, powersave_mode, server_mode: Option<Vec<String>>} }`;
  default config: `default_sched = None`, `default_mode = Auto`
