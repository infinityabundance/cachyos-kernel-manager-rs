//! `cachyos-kernel-manager-strings` — the candidate's user-visible string
//! table, rendered as the courtable descriptor (`cachyos-km-strings-v1`)
//! for the `ui/dialog-strings` court. The table is the single source of
//! truth in `crates/cachyos-kernel-manager-ui/src/strings.rs`;
//! every string is quoted from the frozen source with its file reference.
//!
//! Usage: cachyos-kernel-manager-strings

use cachyos_kernel_manager_ui::strings::inventory;
use serde_json::json;
use std::process::ExitCode;

fn main() -> ExitCode {
    let rows: Vec<serde_json::Value> = inventory()
        .iter()
        .map(|(id, source, text)| json!({ "id": id, "source": source, "text": text }))
        .collect();
    let payload = json!({ "schema": "cachyos-km-strings-v1", "strings": rows });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize")
    );
    ExitCode::SUCCESS
}
