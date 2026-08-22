# build-env/makepkg-runtime

gap-006 differential VM court: the **runtime** dependency-resolution
semantics of the oracle's build commands. The command CONSTRUCTION is
courted by `build-env/lifecycle` + `aur/enablement-matrix`; this court
executes both command sets under strace and compares the exec chains.

- **Oracle side** (`vm/in-vm/oracle-makepkg.sh`): the frozen source's
  literal strings — `makepkg -scf --cleanbuild --skipchecksums && touch
  .done-status` (`conf-window.cpp:734`) and `makepkg -sicf
  --cleanbuild --skipchecksums` (`aur_kernel.cpp:53`) — on the fixture's
  build projects;
- **Candidate side** (`vm/in-vm/candidate-makepkg.sh`): the same strings
  RENDERED BY THE CANDIDATE'S MODEL (`cachyos-kernel-manager-buildcmd`:
  `BuildFlowPlan::render` + `makepkg_aur_argv`).

The witness pins (all three scenarios byte-identical between the sides):

1. **-scf** resolves the repo dep `km-runtime-dep` via
   `sudo pacman -S --asdeps km-runtime-dep` (makepkg `-s`, --syncdeps) and
   does NOT install the built package;
2. **-sicf** resolves the same dep AND installs the built package via
   `sudo pacman -U <artifact>` (makepkg `-i`) — the `-scf` vs `-sicf`
   difference is exactly the `-i` step;
3. an **AUR-only dep** (`km-aur-only-dep`, resolvable nowhere) fails the
   `-s` resolution identically for both commands (makepkg can only install
   deps from the sync repos).

`commands.txt` (the literals vs the model render) must also be identical.
The fixture (`vm/fixtures/makepkg-runtime`) provides `km-runtime-dep`,
passwordless sudo for the `test` user, and the two PKGBUILD projects.

Status: defined. Execution:

```
cargo xtask vm bake makepkg-runtime
cargo build -p cachyos-kernel-manager-build --bin cachyos-kernel-manager-buildcmd
cargo xtask court run build-env/makepkg-runtime --vm
```

Falsifier: any execve chain difference in any scenario, any
`commands.txt` difference, or any machine-residual difference.
