//! `cachyos-kernel-manager-scx-introspect` — the candidate's typed
//! `org.scx.Loader` interface, rendered as the courtable descriptor
//! (`cachyos-km-scx-interface-v1`). The `scx/loader-interface` court
//! byte-compares this against the oracle reference (source-derived from
//! `scx_loader 1.0.9`) and, in the VM court, against the real
//! `busctl introspect org.scx.Loader`.
//!
//! Usage: cachyos-kernel-manager-scx-introspect
//! (no arguments; the interface is fixed by the type system)

use cachyos_kernel_manager_scx::loader_interface;
use serde_json::json;
use std::process::ExitCode;

fn main() -> ExitCode {
    let iface = loader_interface();
    let methods: Vec<serde_json::Value> = iface
        .methods
        .iter()
        .map(|m| {
            json!({
                "name": m.name,
                "in_args": m.in_args.iter().map(|(n, t)| json!({"name": n, "type": t})).collect::<Vec<_>>(),
                "out_args": m.out_args,
            })
        })
        .collect();
    let properties: Vec<serde_json::Value> = iface
        .properties
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "type": p.signature,
                "access": if p.read && p.write { "readwrite" } else if p.read { "read" } else { "write" },
            })
        })
        .collect();
    let payload = json!({
        "schema": "cachyos-km-scx-interface-v1",
        "interface": iface.interface,
        "service": iface.service,
        "path": iface.path,
        "methods": methods,
        "properties": properties,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
