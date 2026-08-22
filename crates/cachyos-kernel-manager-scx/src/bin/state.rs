//! `cachyos-kernel-manager-scx-state` — the candidate's typed client reading
//! the REAL `org.scx.Loader` properties over the system bus (the
//! `scx/state-readback` VM court's candidate side). Renders the readback in
//! the court schema (`cachyos-km-scx-readback-v1`): the running scheduler,
//! the mode, and the supported list — exactly what the oracle's window
//! reads (`get_current_sched`/`get_current_mode`/`get_supported_scheds`).
//!
//! Usage: cachyos-kernel-manager-scx-state

use cachyos_kernel_manager_scx::client::LoaderClientProxy;
use serde_json::json;
use std::process::ExitCode;
use zbus::Connection;

fn main() -> ExitCode {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to initialize tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = rt.block_on(async {
        let connection = Connection::system().await?;
        let loader = LoaderClientProxy::new(&connection).await?;
        let current = loader.current_scheduler().await?;
        let mode = loader.scheduler_mode().await?;
        let supported = loader.supported_schedulers().await?;
        Ok::<_, zbus::Error>(json!({
            "schema": "cachyos-km-scx-readback-v1",
            "current_scheduler": current,
            "scheduler_mode": mode.as_u8(),
            "supported_schedulers": supported,
        }))
    });
    match result {
        Ok(payload) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).expect("serialize")
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("scx state readback failed: {e}");
            ExitCode::FAILURE
        }
    }
}
