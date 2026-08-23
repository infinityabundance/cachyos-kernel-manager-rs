//! Offscreen layout previews — render the REAL window components with dummy
//! data to PNGs so the layout can be verified by eye without the VM
//! round-trip.
//!
//! Safety: this test NEVER touches the kernel logic. It instantiates the
//! three Slint windows with fabricated rows/combos/patches, renders them
//! headlessly (i-slint-backend-testing + the software renderer), and writes
//! PNGs to `target/layout-preview/`. No libalpm, no probes, no pacman.

#![cfg(feature = "rendering")]

use cachyos_kernel_manager_ui::app::{
    ConfCheckRow, ConfigureWindow, MainWindow, SchedExtWindow, TreeRow,
};
use slint::ComponentHandle;

fn init_headless() {
    use i_slint_backend_testing::testing_backend::{TestingBackend, TestingBackendOptions};
    i_slint_core::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
        mock_time: true,
        threading: false,
        renderer_name: Some("software".into()),
        ..Default::default()
    })))
    .expect("platform already initialized");
}

fn save(window: &slint::Window, name: &str) {
    let mut buf = window.take_snapshot().expect("snapshot");
    let w = buf.width();
    let h = buf.height();
    // SharedPixelBuffer<Rgba8Pixel>: flatten the per-pixel RGBA to bytes
    let raw: Vec<u8> = buf
        .make_mut_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();
    let img = image::RgbaImage::from_raw(w, h, raw).expect("rgba");
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("..")
        .join("layout-preview");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    img.save(&path).unwrap();
    println!("preview saved: {} ({}x{})", path.display(), w, h);
}

/// Dump the geometry of every element matching `pred` (by accessible label or
/// type), as `label: x,y w×h`. Lets the layout be verified numerically.
#[allow(dead_code)] // debugging aid; the assertions use `find_one`
fn dump<C: slint::ComponentHandle>(
    root: &C,
    label: &str,
    matcher: impl Fn(&i_slint_backend_testing::ElementHandle) -> bool + 'static,
) {
    use i_slint_backend_testing::ElementQuery;
    let found = ElementQuery::from_root(root).match_predicate(matcher).find_all();
    for e in found {
        let p = e.absolute_position();
        let s = e.size();
        println!(
            "  [{}] {} at x={:.0} y={:.0} size={:.0}x{:.0}",
            label, e.id().unwrap_or_default(), p.x, p.y, s.width, s.height
        );
    }
}

/// Find one element by predicate, return its (x, y, w, h).
fn find_one<C: slint::ComponentHandle>(
    root: &C,
    matcher: impl Fn(&i_slint_backend_testing::ElementHandle) -> bool + 'static,
) -> (f32, f32, f32, f32) {
    use i_slint_backend_testing::ElementQuery;
    let e = ElementQuery::from_root(root)
        .match_predicate(matcher)
        .find_first()
        .unwrap_or_else(|| panic!("element not found"));
    let p = e.absolute_position();
    let s = e.size();
    (p.x, p.y, s.width, s.height)
}

fn by_accessible_label(label: &str) -> impl Fn(&i_slint_backend_testing::ElementHandle) -> bool {
    let label = label.to_string();
    move |e| e.accessible_label().is_some_and(|l| l.as_str() == label)
}

fn main_window_preview() {
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1000., 700.));
    ui.set_description(
        "Select the kernels you want to install. Only one kernel can be set as the default boot option at a time."
            .into(),
    );
    ui.set_label_choose("Choose".into());
    ui.set_label_pkgname("PkgName".into());
    ui.set_label_version("Version".into());
    ui.set_label_category("Category".into());
    ui.set_label_execute("Execute".into());
    ui.set_label_configure("Configure".into());
    ui.set_label_cancel("Cancel".into());
    ui.set_label_schedext("sched-ext scheduler config".into());
    ui.set_schedext_visible(true);
    let rows = vec![
        TreeRow {
            raw: "cachyos/linux-cachyos".into(),
            version: "7.2.0-1".into(),
            category: "stable".into(),
            checked: true,
            immutable: true,
        },
        TreeRow {
            raw: "cachyos/linux-cachyos-bore".into(),
            version: "7.1.8-1".into(),
            category: "stable".into(),
            checked: false,
            immutable: false,
        },
        TreeRow {
            raw: "cachyos-znver4/linux-cachyos-rc".into(),
            version: "7.2.rc7-1".into(),
            category: "release candidate".into(),
            checked: false,
            immutable: false,
        },
        TreeRow {
            raw: "extra/linux-hardened".into(),
            version: "7.1.9.hardened1-1".into(),
            category: "hardened kernel".into(),
            checked: true,
            immutable: false,
        },
    ];
    ui.set_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    save(&ui.window(), "main.png");
    // the main window's button heights — the SCX dialog's buttons must match
    // exactly (the user spec: same vertical height as the main window)
    println!("Main window (1000x700):");
    let exec = find_one(&ui, by_accessible_label("Execute"));
    let conf = find_one(&ui, by_accessible_label("Configure"));
    let cancel = find_one(&ui, by_accessible_label("Cancel"));
    println!(
        "  execute {}x{}, configure {}x{}, cancel {}x{}",
        exec.2, exec.3, conf.2, conf.3, cancel.2, cancel.3
    );
}

fn scx_window_preview() {
    let w = SchedExtWindow::new().unwrap();
    w.window().set_size(slint::LogicalSize::new(480., 320.));
    w.set_label_running("Running scheduler:".into());
    w.set_running("scx_bpfland".into());
    w.set_label_scheduler("Select scheduler:".into());
    w.set_schedulers(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("scx_bpfland"),
        slint::SharedString::from("scx_lavd"),
        slint::SharedString::from("scx_rusty"),
    ])));
    w.set_scheduler_index(0);
    w.set_label_profile("Select profile:".into());
    w.set_profiles(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("performance"),
        slint::SharedString::from("powersave"),
    ])));
    w.set_profile_index(0);
    w.set_profile_visible(true);
    w.set_label_flags("Set flags:".into());
    w.set_flags("--slice-smooth 1".into());
    w.set_enabled(true);
    w.set_label_apply("Apply".into());
    w.set_label_disable("Disable".into());
    w.set_label_cancel("Cancel".into());
    save(&w.window(), "scx.png");
    // the window minimum: the user requirement (can grow, never shrink below
    // the default). The winit adapter reads this via layout_constraints() and
    // turns it into min_inner_size.
    // the window minimum: the user requirement (can grow, never shrink below
    // the default). Replicate the winit adapter's constraint read: the root
    // component's layout_info min/max (min_max_size_for_layout_constraints).
    let inner = i_slint_core::window::WindowInner::from_pub(&w.window());
    let component_rc = inner.component();
    let component = i_slint_core::item_tree::ItemTreeRc::borrow_pin(&component_rc);
    let h = component.as_ref().layout_info(i_slint_core::layout::Orientation::Horizontal);
    let v = component.as_ref().layout_info(i_slint_core::layout::Orientation::Vertical);
    let (h_min, v_min) = (h.min as f32, v.min as f32);
    println!("  root layout_info: h.min={h_min} v.min={v_min}");
    assert!(h_min >= 479.0, "SCX min width enforced: {h_min}");
    assert!(v_min >= 319.0, "SCX min height enforced: {v_min}");
    // geometry verification: combo/lineedit widths + button order/position
    println!("SCX window (480x320):");
    println!("  -- all scx elements --");
    dump(&w, "all", |_| true);
    // both combos: normal width AND identical height (the `if`-wrapped row
    // used to stretch one combo to 46px while the other stayed 32px)
    let mut combos: Vec<(f32, f32)> = i_slint_backend_testing::ElementQuery::from_root(&w)
        .match_predicate(|e| {
            e.type_name().is_some_and(|t| t.contains("ComboBox"))
                && e.id().is_none_or(|i| i.is_empty())
        })
        .find_all()
        .into_iter()
        .map(|e| {
            let s = e.size();
            (s.width, s.height)
        })
        .collect();
    assert_eq!(combos.len(), 2, "two combos");
    for (i, (wdt, hgt)) in combos.iter().enumerate() {
        assert!((wdt - 220.0).abs() < 8.0, "combo {i} width: {wdt}");
        assert!(*hgt <= 34.0, "combo {i} height (must not inflate): {hgt}");
    }
    assert!((combos[0].1 - combos[1].1).abs() < 2.0, "combos same height");
    println!("  -- scx elements y in 90..180 --");
    dump(&w, "mid", |e| {
        let p = e.absolute_position();
        p.y > 90.0 && p.y < 180.0
    });
    let lineedit = find_one(&w, |e| {
        e.type_name().is_some_and(|t| t.contains("LineEdit")) && e.id().is_none_or(|i| i.is_empty())
    });
    assert!((lineedit.2 - 300.0).abs() < 8.0, "flags width: {}", lineedit.2);
    // buttons: Cancel then Disable then Apply, right-aligned, natural height,
    // uniform min-width (none squashed)
    let cancel = find_one(&w, by_accessible_label("Cancel"));
    let disable = find_one(&w, by_accessible_label("Disable"));
    let apply = find_one(&w, by_accessible_label("Apply"));
    assert!(cancel.0 < disable.0 && disable.0 < apply.0, "button order");
    assert!((apply.1 - cancel.1).abs() < 1.0, "buttons same row");
    assert!(cancel.2 >= 84.0, "cancel width {}", cancel.2);
    assert!(disable.2 >= 84.0, "disable width {}", disable.2);
    assert!(cancel.3 <= 36.0, "button height {}", cancel.3);
    assert!(apply.0 + apply.2 > 430.0, "buttons right-aligned");
    // the SCX buttons must be the SAME height as the main window's buttons
    // (the user spec; the un-stretched rows keep them natural instead of
    // inflating like the old layout did)
    let main = MainWindow::new().unwrap();
    main.set_label_execute("Execute".into());
    main.set_label_configure("Configure".into());
    main.set_label_cancel("Cancel".into());
    main.set_rows(slint::ModelRc::new(slint::VecModel::from(Vec::<TreeRow>::new())));
    let _ = main.window().take_snapshot().expect("main snapshot"); // force layout
    let main_h = find_one(&main, by_accessible_label("Execute")).3;
    for (lbl, scx_h) in [("cancel", cancel.3), ("disable", disable.3), ("apply", apply.3)] {
        assert!(
            (scx_h - main_h).abs() < 2.0,
            "scx {lbl} height {scx_h} != main {main_h}"
        );
    }
    println!("  buttons match main window height ({main_h:.0}px)");
}

fn configure_options_preview() {
    let w = ConfigureWindow::new().unwrap();
    w.window().set_size(slint::LogicalSize::new(900., 900.));
    configure_common(&w);
    w.set_tab_options(true);
    save(&w.window(), "configure-options.png");
    // geometry verification: NO big gaps — the tcp_bbr3 checkbox must sit
    // directly above the first combo, and the last combo directly above the
    // ZFS checkbox (the old visible-filter left clip-sized holes here)
    println!("Configure Options tab (900x900):");
    let tcpbbr = find_one(&w, by_accessible_label("Enable TCP_CONG_BBR3"));
    let hz = find_one(&w, by_accessible_label("Running tick rate"));
    let lto = find_one(&w, by_accessible_label("Enable LTO"));
    let zfs = find_one(&w, by_accessible_label("Build the ZFS module"));
    // tcp_bbr3's row bottom must be within a normal spacing of the hz combo
    let gap1 = hz.1 - (tcpbbr.1 + tcpbbr.3);
    let gap2 = zfs.1 - (lto.1 + lto.3);
    assert!(gap1 >= 0.0 && gap1 < 40.0, "gap tcpbbr->hz: {gap1}");
    assert!(gap2 >= 0.0 && gap2 < 40.0, "gap lto->zfs: {gap2}");
    println!("  gaps ok: tcpbbr->hz {gap1:.0}px, lto->zfs {gap2:.0}px");
}

fn configure_patches_preview() {
    let w = ConfigureWindow::new().unwrap();
    w.window().set_size(slint::LogicalSize::new(900., 900.));
    configure_common(&w);
    w.set_tab_options(false);
    save(&w.window(), "configure-patches.png");
    // geometry verification: the add buttons must be near the window BOTTOM
    // (y close to 900) and the icon buttons clustered tightly directly above
    // them, right-aligned
    println!("Configure Patches tab (900x900):");
    let add = find_one(&w, by_accessible_label("Add remote patch"));
    assert!(add.1 > 780.0, "add buttons near bottom, got y={}", add.1);
    // the three 26x26 icon buttons: y directly above the add row, clustered
    let mut icons: Vec<f32> = i_slint_backend_testing::ElementQuery::from_root(&w)
        .match_predicate(|e| {
            let s = e.size();
            (s.width - 26.0).abs() < 1.0
                && (s.height - 26.0).abs() < 1.0
                && e.id().is_some_and(|i| i.ends_with("::i-touch"))
        })
        .find_all()
        .into_iter()
        .map(|e| e.absolute_position().x)
        .collect();
    icons.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(icons.len(), 3, "three icon buttons");
    let spread = icons[2] - icons[0];
    assert!(spread < 70.0, "icons clustered, spread={spread}");
    let icon_y = find_one(&w, |e| {
        let s = e.size();
        (s.width - 26.0).abs() < 1.0
            && (s.height - 26.0).abs() < 1.0
            && e.id().is_some_and(|i| i.ends_with("::i-touch"))
    });
    let dy = add.1 - icon_y.1;
    assert!(dy > 20.0 && dy < 60.0, "icons directly above add row, dy={dy}");
    println!("  footer ok: icons spread {spread:.0}px, dy(icons->add) {dy:.0}px");
}

fn configure_common(w: &ConfigureWindow) {
    w.set_variant_labels(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("CachyOS default Scheduler (tuned EEVDF)"),
        slint::SharedString::from("BORE - Burst-Oriented Response Enhancer"),
    ])));
    w.set_variant_index(0);
    let mk = |label: &str, checked: bool| ConfCheckRow {
        label: label.into(),
        checked,
        enabled: true,
    };
    w.set_checks_top(slint::ModelRc::new(slint::VecModel::from(vec![
        mk("Enable CachyOS config", true),
        mk("Tweak kernel options prior to a build via nconfig", false),
        mk("Tweak kernel options prior to a build via xconfig", false),
        mk("Use Modprobed-db", false),
        mk("Use the current kernel's config", false),
        mk("Enable KBUILD_CFLAGS -O3", true),
        mk("Set performance governor as default", false),
        mk("Enable TCP_CONG_BBR3", false),
    ])));
    w.set_checks_bottom(slint::ModelRc::new(slint::VecModel::from(vec![
        mk("Build the ZFS module", false),
        mk("Build the open NVIDIA module", false),
        mk("Include vmlinux with debug informations/symbols", false),
    ])));
    w.set_lto_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("None"),
        slint::SharedString::from("Full"),
        slint::SharedString::from("Thin"),
    ])));
    w.set_lto_index(2);
    w.set_preempt_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("Full"),
        slint::SharedString::from("Lazy"),
    ])));
    w.set_preempt_index(0);
    w.set_hz_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("1000HZ"),
        slint::SharedString::from("750Hz"),
        slint::SharedString::from("100Hz"),
    ])));
    w.set_hz_index(0);
    w.set_tickless_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("Full"),
        slint::SharedString::from("Idle"),
    ])));
    w.set_tickless_index(0);
    w.set_hugepage_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("Always"),
        slint::SharedString::from("Madvise"),
    ])));
    w.set_hugepage_index(0);
    w.set_cpuopt_items(slint::ModelRc::new(slint::VecModel::from(vec![
        slint::SharedString::from("Manual"),
        slint::SharedString::from("Native"),
        slint::SharedString::from("Zen4"),
    ])));
    w.set_cpuopt_index(0);
    w.set_custom_name("$pkgbase-custom".into());
    w.set_patches(slint::ModelRc::new(slint::VecModel::from(vec![slint::SharedString::from(
        "https://raw.githubusercontent.com/cachyos/kernel-patches/master/7.2/misc/dkms-clang.patch",
    )])));
    w.set_selected_patch(0);
    w.set_label_variant("Select kernel".into());
    w.set_label_custom_name("Custom package name".into());
    w.set_label_hz("Running tick rate".into());
    w.set_label_tickless("Select tickless".into());
    w.set_label_preempt("Select preempt".into());
    w.set_label_hugepage("Transparent Hugepages".into());
    w.set_label_cpuopt("CPU compiler optimizations".into());
    w.set_label_lto("Enable LTO".into());
    w.set_label_tab_options("Options".into());
    w.set_label_tab_patches("Patches".into());
    w.set_label_add_local("Add local patch".into());
    w.set_label_add_remote("Add remote patch".into());
    w.set_label_remove("Remove".into());
    w.set_label_up("Move up".into());
    w.set_label_down("Move down".into());
    w.set_label_save("Save".into());
    w.set_label_load("Load".into());
    w.set_label_cancel("Cancel".into());
    w.set_label_execute("Build kernel".into());
}

#[test]
fn preview_all_windows() {
    init_headless();
    main_window_preview();
    scx_window_preview();
    configure_options_preview();
    configure_patches_preview();
    // sanity: the snapshots are non-blank and reasonably sized
    for name in ["main.png", "scx.png", "configure-options.png", "configure-patches.png"] {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("..")
            .join("layout-preview");
        let img = image::open(dir.join(name)).expect("png");
        assert!(img.width() > 100 && img.height() > 100, "{name} too small");
    }
}
