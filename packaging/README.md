# packaging/

Drop-in Arch/CachyOS packaging (Phase 10):

- PKGBUILD for `cachyos-kernel-manager` (binary, desktop entry, icons,
  polkit policy, privileged helper + shim, translations)
- install layout preserving the oracle's identity surfaces
  (docs/COMPATIBILITY.md)
- upgrade/revert courts: C++ package → Rust package → launch → normal
  operation; and back (file-collision court, `pacman -Ql` comparison)

Nothing here yet — Phase 10.
