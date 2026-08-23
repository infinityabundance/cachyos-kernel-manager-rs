//! Compile the Slint UI definitions at build time. The shared dialog overlay
//! (`dialogs.slint`) is imported by the window files — the compiler inlines
//! it, so it is not compiled as its own module.

fn main() {
    // debug info enables the ElementHandle introspection API (used by the
    // layout-preview integration test to verify window geometry numerically;
    // negligible runtime cost, no layout effect).
    std::env::set_var("SLINT_EMIT_DEBUG_INFO", "1");
    slint_build::compile("ui/main_window.slint").expect("slint build failed");
    slint_build::compile("ui/configure_window.slint").expect("slint build failed");
    slint_build::compile("ui/scx_window.slint").expect("slint build failed");
}
