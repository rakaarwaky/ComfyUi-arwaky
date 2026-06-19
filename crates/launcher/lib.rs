// PURPOSE: Library root — surface layer. Tauri commands call aggregate only.
// Re-exports sub-crates for binary targets.

pub use launcher_backend_engine;
pub use launcher_config;
pub use launcher_engine;
pub use launcher_gpu_detector;
pub use launcher_log_engine;
pub use launcher_process_manager;
pub use launcher_shared;

pub mod root_launcher_container;

use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use tauri::{Emitter, Manager};

use crate::root_launcher_container::DownloadState;
use launcher_shared::contract_launcher_aggregate::LauncherAggregate;
use launcher_shared::BackendStatus;

// ── Surface: Tauri commands (thin wrappers calling aggregate) ──

#[tauri::command]
fn check_backend_status(agg: tauri::State<'_, Arc<dyn LauncherAggregate>>) -> BackendStatus {
    agg.check_backend_status()
}

#[tauri::command]
fn start_backend_download(
    app_handle: tauri::AppHandle,
    agg: tauri::State<'_, Arc<dyn LauncherAggregate>>,
) {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    {
        let state = app_handle.state::<DownloadState>();
        *state.cancel_token.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel.clone());
        state
            .is_downloading
            .store(true, std::sync::atomic::Ordering::Release);
    }

    let app_handle_progress = app_handle.clone();
    let app_handle_error = app_handle.clone();
    agg.start_backend_download(
        cancel,
        Box::new(move |event| {
            let _ = app_handle_progress.emit("comfyui-download-progress", &event);
        }),
        Box::new(move || {
            let _ = app_handle.emit("comfyui-download-complete", ());
        }),
        Box::new(move |e| {
            let _ = app_handle_error.emit("comfyui-download-error", &e);
        }),
    );
}

#[tauri::command]
fn cancel_backend_download(agg: tauri::State<'_, Arc<dyn LauncherAggregate>>) {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(true));
    agg.cancel_backend_download(&cancel);
}

#[tauri::command]
fn start_comfyui(
    app_handle: tauri::AppHandle,
    agg: tauri::State<'_, Arc<dyn LauncherAggregate>>,
) -> Result<(), String> {
    // Get the log sender from managed state
    let log_tx = {
        let sender = app_handle.state::<launcher_shared::LogSender>();
        let guard = sender.tx.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    let log_tx: SyncSender<launcher_shared::LogMessage> = match log_tx {
        Some(tx) => tx,
        None => return Err("LogSender already dropped".to_string()),
    };

    // Emit exit event via Tauri
    let app_exit = app_handle.clone();
    agg.start_comfyui(
        log_tx,
        Box::new(move || {
            let _ = app_exit.emit("comfyui-exited", ());
        }),
    )
    .map_err(|e| e.to_string())
}

pub fn run() {
    std::env::set_var("no_proxy", "*");
    std::env::set_var("NO_PROXY", "*");

    let (log_tx, log_rx) = std::sync::mpsc::sync_channel::<launcher_shared::LogMessage>(
        launcher_log_engine::LOG_CHANNEL_CAPACITY,
    );

    let app = root_launcher_container::configure_app(log_tx, log_rx)
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Enable devtools for debugging
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }

    app.run(|_app_handle, _event| {
        // Surface has no direct cleanup — process-manager handles lifecycle
    });
}
