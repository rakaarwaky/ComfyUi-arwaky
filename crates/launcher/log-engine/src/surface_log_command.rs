// PURPOSE: Tauri surface commands for log retrieval — expose log buffer, health, GPU metrics.

use std::sync::atomic::Ordering;
use tauri::Manager;
use tauri::State;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

use launcher_shared::contract_gpu_monitor_port::GpuMonitorPort;
use launcher_shared::{HealthState, LogBuffer, LogStats};

/// Return logs newer than `last_id`, plus the new max_id for polling.
#[tauri::command]
pub fn get_logs(
    log_buffer: State<'_, LogBuffer>,
    last_id: Option<u64>,
) -> (Vec<(u64, String)>, u64) {
    let logs = match log_buffer.logs.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let last_id = last_id.unwrap_or(0);
    let new_logs: Vec<_> = logs
        .iter()
        .filter(|(id, _)| *id > last_id)
        .cloned()
        .collect();

    let max_id = new_logs.iter().map(|(id, _)| *id).max().unwrap_or(last_id);

    (new_logs, max_id)
}

/// Return (total_received, dropped) counters for log health monitoring.
#[tauri::command]
pub fn get_log_stats(stats: State<'_, LogStats>) -> (u64, u64) {
    (
        stats.total_received.load(Ordering::Relaxed),
        stats.dropped.load(Ordering::Relaxed),
    )
}

/// Return full system health snapshot.
#[tauri::command]
pub fn get_health(
    gpu_monitor: State<'_, Box<dyn GpuMonitorPort>>,
    log_stats: State<'_, LogStats>,
    start_time: State<'_, crate::StartTime>,
) -> HealthState {
    let gpu = gpu_monitor.get_metrics();
    let (received, dropped) = (
        log_stats.total_received.load(Ordering::Relaxed),
        log_stats.dropped.load(Ordering::Relaxed),
    );
    let uptime = start_time.0.elapsed().as_secs();

    HealthState {
        backend_alive: false,
        uptime_secs: uptime,
        gpu,
        logs_received: received,
        logs_dropped: dropped,
        backend_pid: None,
    }
}

/// Return latest GPU metrics snapshot.
#[tauri::command]
pub fn get_gpu_metrics(
    gpu_monitor: State<'_, Box<dyn GpuMonitorPort>>,
) -> launcher_shared::GpuMetrics {
    gpu_monitor.get_metrics()
}

/// Open a dedicated log viewer window.
#[tauri::command]
pub fn open_log_viewer(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("log-viewer") {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "log-viewer",
        WebviewUrl::App("log-viewer.html".into()),
    )
    .title("Backend Logs — ComfyUI Desktop")
    .inner_size(900.0, 600.0)
    .min_inner_size(600.0, 400.0)
    .resizable(true)
    .decorations(true)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}
