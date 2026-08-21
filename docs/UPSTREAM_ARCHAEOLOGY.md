# UPSTREAM ARCHAEOLOGY

Findings from full manual archaeology of upstream `6b4a373e` (v1.19.0).
Every claim below is quoted from the frozen source; the authoritative
machine-readable inventory is [`atlas/inventory.json`](../atlas/inventory.json).

## Repository shape (v1.19.0)

```
src/                         C++ sources (Qt6 Widgets GUI + domain logic)
src/*.ui                     Qt Designer files (km-window, conf-window,
                             conf-options-page, conf-patches-page)
src/ini.hpp                  vendored mINI INI parser (the pacman.conf parser!)
src/rootshell.sh             polkit root helper: exec /bin/bash "$@"
src/terminal-helper          Garuda-derived terminal launcher (bash)
src/mkoptions.py             generates compile_options.hpp (frozen maps) from
                             src/compile_options.json
src/compile_options.json     option_name -> PKGBUILD variable map
config-option-lib/           Rust crate (cxxbridge, serde TOML) — the public
                             config file schema; linked via Corrosion+CMake
lang/*.ts                    Qt translations (16 locales; uk not in qrc)
cmake/                       CPM + warnings + linker + sanitizers
subprojects/                 meson fallbacks (fmt)
.github/workflows/           CI (Arch container via pacman on ubuntu runner)
```

Two build systems exist and disagree:
- **CMake** (primary; used by CachyOS packaging): C++23, Qt6 + libalpm +
  glib + PolkitQt6-1 + scxctl-ui + fmt + frozen + Corrosion(Rust lib).
- **meson** (stale): C++20, Qt6 Widgets + fmt + libalpm + glib only; does NOT
  build `config-option-lib`; references `pkg_dummy_impl` which
  `meson_options.txt` does not declare (configure would fail); adds
  `-DENABLE_AUR_KERNELS` via the `aur_kernels` option.

## The Rust/C++ boundary

`config-option-lib` (edition 2024, cxx + serde + toml 1.1, staticlib) exposes:

- `parse_config_file(path) -> Config`
- `parse_config(content) -> Config`
- `write_config_file(config, path)` — plain `File::create` + write, no
  atomicity, no fsync

`Config` (serde, all fields defaulted):

```toml
hardly_check            = bool
per_gov_check           = bool
tcp_bbr3_check          = bool
cachy_config_check      = bool
nconfig_check           = bool
xconfig_check           = bool
localmodcfg_check       = bool
use_current_check       = bool
builtin_zfs_check       = bool
builtin_nvidia_open_check = bool
build_debug_check       = bool
hz_ticks_combo          = string
tickrate_combo          = string
preempt_combo           = string
hugepage_combo          = string
lto_combo               = string
cpu_opt_combo           = string
custom_name_edit        = string
```

Field order above is the serialization order (`toml::to_string` follows struct
declaration order). `ConfigOptions` in C++ mirrors it field-for-field.

## Process model

- Single instance: `QSharedMemory("CachyOS-KM-lock")`; second instance exits -1.
- ALPM handle opened in the `MainWindow` constructor member initializer
  (`parse_alpm("/", "/var/lib/pacman/")`) — i.e. **blocking, on the GUI
  thread**, before the window shows.
- `Kernel::get_kernels` also runs in the constructor (blocking).
- A worker `QThread` runs the transaction loop; the UI thread is woken by a
  condition variable. `on_execute` starts the thread each time (the loop
  persists; `m_running` gates re-entry). `closeEvent` signals shutdown and
  `alpm_release`s **without joining the worker**.
- `prepare_git_repo` calls `fs::current_path` (process-global `chdir`) and
  never restores it — after the first configure, the app's cwd is
  `~/.cache/cachyos-km/pkgbuilds/linux-cachyos` and build paths derive from it.

## Kernel discovery (`Kernel::get_kernels`)

1. For each sync db (order = pacman.conf section order via mINI):
   `alpm_db_search` with the regex needle `linux[^ ]*-headers`.
2. Drop matches containing substring `linux-api-headers`.
3. `headers` = same-db package under the found name; kernel = same-db package
   with the single `-headers` suffix removed; skip if the kernel package is
   absent from that db.
4. Display name (PkgName column) = `"<db>/<kernel>"`; repo = db name;
   installed-db provenance recorded when `HAVE_ALPM_INSTALLED_DB`.
5. Companions (same db): for `linux-cachyos*` → `<pkg>-zfs`,
   `<pkg>-nvidia`, `<pkg>-nvidia-open`; for `linux`/`linux-lts` →
   `nvidia[-lts]`, `nvidia-open[-lts]` (all `linux` substrings removed).
6. AUR (only with `ENABLE_AUR_KERNELS`): requires `/sbin/paru` **and**
   `/sbin/awk` (flipped from `||` to `&&` at 5b075dc); runs
   `paru --aur -Sl | grep ' linux[^ ]*-headers' | awk '{print $2}'`, dedups by
   name against repo kernels, version = `unknown-version`, raw = `aur/<name>`.

## Version state machine

```
repo == aur              -> "unknown-version"            (m_update=false)
not installed            -> sync version
installed:
  vercmp(local, sync):
    > 0  -> "∨<local>"    (downgrade; U+2228)
    < 0  -> "∧<sync>"     (update;  m_update=true; U+2227)
    = 0  -> sync version
```

Sorting on the Version column strips the `∨`/`∧` prefix and compares with
`alpm_pkg_vercmp`. Version comparison is ALPM semantics everywhere — never
semver.

## Category classifier (`Kernel::category`)

Substring scan in this exact order: `lto` → `lto optimized`; `lts` →
`longterm`; `zen` → `zen-kernel`; `hardened` → `hardened kernel`; `deckify` →
`handheld kernel`; `server` → `server kernel`; `next` → `next release`;
`mainline` → `mainline branch`; `git` → `master branch`; `rc` →
`release candidate`; else `stable`. Substring, not prefix; first match wins.

## NVIDIA companion decision matrix (`Kernel::install`)

Inputs (evaluated per kernel at install-list-build time):
- `root_on_zfs` (static, process-lifetime)
- `chwd_nvidia` = any `chwd --list-installed -d 2>/dev/null | grep Name |
  awk '{print $4}'` line starts with `nvidia-dkms` (static, process-lifetime)
- `chwd_nvidia_open` = same, `nvidia-open-dkms` prefix (static)
- local db contains `nvidia-dkms` / `nvidia-open-dkms`
- `pacman -Qqs '^linux-cachyos.*-nvidia$'` / `-nvidia-open$` non-empty
  (NOTE: these two regexes only match linux-cachyos-nvidia packages, so a
  plain `linux`-family prebuilt nvidia is not detected by them)

Logic:

```
if zfs_root && zfs_module_known:          add zfs module            [1]
nvidia_open_pref = chwd_open && open_module_known
nvidia_pref     = chwd_nvidia && nvidia_module_known
if open_modules_installed && open_module_known:   nvidia_open_pref=true;  nvidia_pref=false
elif nvidia_modules_installed && nvidia_module_known: nvidia_pref=true; nvidia_open_pref=false
if !dkms_installed && nvidia_open_pref:  add open module            [2]
elif !dkms_installed && nvidia_pref:     add nvidia module          [3]
add kernel, add headers                                               [4]
```

Order in install list: `[zfs?] [nvidia?] [kernel] [headers]`.
AUR kernels bypass this entirely (makepkg path, no companions).

## ZFS detection

`findmnt -ln -o FSTYPE /` output compared exactly to `zfs` (through
`popen`/`/bin/sh -c`), evaluated once at static init. Any other value
(including failure → empty string, or `-1` on popen failure) means not-ZFS.

## Transaction rendering

- install: `pacman -S --needed <joined list>` (escalated terminal)
- remove: `pacman -Rsn <joined list>` (escalated terminal)
- AUR: per-kernel `makepkg -sicf --cleanbuild --skipchecksums` (non-escalated)
  after git clone/refresh of `https://aur.archlinux.org/<pkg>.git`
- All join with single spaces; package names come from libalpm, not user input
  (except custom pkgbase, which never reaches pacman -S).

## Build configuration subsystem

Variant dirs: `cachyos→linux-cachyos`, `bmq→linux-cachyos-bmq`,
`bore→linux-cachyos-bore`, `hardened→linux-cachyos-hardened`,
`lts→linux-cachyos-lts`, `rc→linux-cachyos-rc`, `rt→linux-cachyos-rt-bore`,
`eevdf→linux-cachyos-eevdf`, `deckify→linux-cachyos-deckify`,
`server→linux-cachyos-server`, fallback `linux-cachyos`.

Value lists (internal ids, in combo order):
- hz: `1000 750 600 500 300 250 100` (labels `1000HZ 750Hz 600Hz 500Hz 300Hz
  250Hz 100Hz` — note the casing inconsistency)
- tickless: `full idle periodic` (`Full Idle Periodic`)
- preempt: `full lazy voluntary none` (`Full Lazy Voluntary None`; the last
  two added only for lts/hardened)
- lto: `none full thin thin-dist` (`No Full Thin Thin-dist`; thin-dist added
  only for non-lts/non-hardened)
- hugepage: `always madvise` (`Always Madvise`)
- cpu_opt: `manual native generic_v1 generic_v2 generic_v3 generic_v4 zen4`
  (`Disabled Native CPU Generic / x86_64 x86_64_v2 x86_64_v3 x86_64_v4 Zen4`)

Defaults at window open: cachyconfig=on, hardly=on, lto=thin, hz=1000,
tickless=full, preempt=full, cpu_opt=disabled(manual), hugepage=always,
custom_name=`$pkgbase-custom`.

Variant-switch resets (in order): lto item availability + default
(thin for cachyos/rc else none), preempt item availability + default (lazy
for server else full), hz default (300 server else 1000), cachyconfig
(unchecked server), builtin_zfs (disabled+unchecked for rt), patches tab
refresh.

Env var rendering (`get_all_set_values`), one line per var, `\n` separated:
11 checkboxes → `_<var>=yes|no`; combos → `_HZ_ticks`, `_tickrate`,
`_preempt`, `_hugepage`, `_lto` = value; `_processor_opt` only when
cpu_opt != manual; plus `_use_lto_suffix=n` when lto != none && custom_name
!= `$pkgbase` (workaround for PKGBUILD custom-pkgname breakage).

PKGBUILD mutation:
1. `source=(...)` block: every original entry not ending `.patch`, quoted
   `"..."`, followed by each patch-list item, quoted; joined with `\n`;
   rendered `source=(\n...)\n`; inserted before `prepare()` (at the last
   newline before it). The original `source=(...)` is left untouched — the
   later assignment wins in bash. No-op if `prepare()` (or the preceding
   newline) is absent.
2. `pkgbase="<custom_name>"` preceded by `\n\n`, inserted before `_major=` at
   the last newline. No-op if `_major=` absent.

Both are plain text inserts into the PKGBUILD file (non-atomic writes via
`write_to_file`). The candidate must reproduce byte-identical residuals for
identical inputs (courts in `courts/patch-injection/`, `courts/custom-name/`).

Build lifecycle:
- `makepkg -scf --cleanbuild --skipchecksums && touch .done-status` run in a
  non-escalated terminal with cwd = `<app cwd>/<variant dir>`.
- Success = `.done-status` exists when the terminal process finishes (exit
  code is NOT the success signal; the helper's exit-code semantics are
  unreliable anyway).
- Install prompt: `sudo pacman -U <globs>` where globs =
  `<pkgfunc-suffix>-<pkgver>-<pkgrel>-*<PKGEXT>` for each `package_*`
  function from `declare -F`; `pkgver-pkgrel` from the sourced PKGBUILD;
  PKGEXT from `/etc/makepkg.conf` (fallback `.pkg.tar.zst`).

## AUR support

Feature flag `ENABLE_AUR_KERNELS` (meson `aur_kernels`, default off). At
discovery: `paru --aur -Sl | grep ' linux[^ ]*-headers' | awk '{print $2}'`.
Cache: `~/.cache/cachyos-km/aur_pkgbuilds/<pkg>`. Build cache and repo cache
are distinct directories — a documented compatibility surface.

## SCX integration

The main window embeds `scxctl::SchedExtWindow` (external `scxctl-ui`
library, D-Bus `org.scx.Loader`). The sched-ext button is hidden unless
`/sys/kernel/sched_ext/state` exists. History: scx-manager was extracted from
this repo; D-Bus apply/disable logic was moved into Rust
(commits `425681d`, `c866d99`, `780b9b1`). Phase 7 reconstructs the typed
Rust client against the real interface.

## Notable strings / paths

- Caches: `~/.cache/cachyos-km`, `~/.cache/cachyos-km/pkgbuilds`,
  `~/.cache/cachyos-km/aur_pkgbuilds`
- Repo URL: `https://github.com/cachyos/linux-cachyos.git`
- Helpers: `/usr/lib/cachyos-kernel-manager/terminal-helper`,
  `/usr/lib/cachyos-kernel-manager/rootshell.sh`
- PKGEXT probe: `.testscriptpkgext` in cwd; source-array probe:
  `.testscript` in variant dir; function probe: `.testscriptpkgnames` in
  variant dir; success marker: `.done-status` in variant dir
- Version-markers: `∨` (U+2228), `∧` (U+2227)
