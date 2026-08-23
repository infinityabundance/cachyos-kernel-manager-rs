# ui/i18n-rendered

Phase 12 hostile-review rendered-i18n court (the audit's P2 requirement:
"your i18n court needs to include rendered production projections, not
just catalog lookup" + "keep an explicit CJK rendering court").

The court participant is the packaged GUI itself:

1. the runner stages the release binary (`target/release/cachyos-kernel-
   manager`, `--features gui-alpm`) into the 9p share;
2. boots a fresh overlay of the `i18n-rendered` fixture (the
   gui-integration X11/AT-SPI stack + GENERATED de_DE.UTF-8 and
   zh_CN.UTF-8 locales) per side;
3. **oracle side**: `oracle-i18n-drive.sh` launches the frozen Qt binary
   under LANG=de_DE.UTF-8 (then zh_CN.UTF-8) under Xvfb + AT-SPI;
4. **candidate side**: `candidate-i18n-drive.sh` launches the release
   Slint binary (SLINT_BACKEND=winit-software + the org.a11y.Status stub)
   under the same locales;
5. both run the SAME side-agnostic driver (`i18n-drive.py`): project the
   main window's translation-sensitive chrome — the window title, the
   description, the four tree column headers, the action buttons;
6. compares `i18n-de_DE.json` + `i18n-zh_CN.json` byte-for-byte + the
   machine residuals.

## What each locale witnesses

- **de_DE** — the translated production projection: the description and
  the four headers resolve through the catalogs (German: PkgName →
  Paketname, Category → Kategorie). This is the regression the audit P2
  found: the old Slint sync fed the English constants straight through.
- **zh_CN** — gap-009's rendered projection: BOTH sides render English,
  because Qt's QLocale reports `zh_CN` while the frozen qrc alias is
  `zh-CN` — the oracle never loads its CJK catalog. The candidate
  reproduces the miss (ui/i18n-resolution). The non-Latin
  glyph-rendering question (slint's software renderer limits text to
  western scripts) is therefore NOT reachable through the shipped app's
  locale resolution — this court records that rendered projection.

## Run

```sh
cargo build --release --features gui-alpm
cargo xtask vm bake i18n-rendered
cargo xtask court run ui/i18n-rendered --vm
```

## Falsifiers

Any byte difference in the rendered projection for either locale, or any
machine-residual difference between the two boots.
