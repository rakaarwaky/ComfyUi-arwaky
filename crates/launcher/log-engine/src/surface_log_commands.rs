// PURPOSE: Tauri surface commands for log retrieval — expose log buffer to frontend.

use std::sync::atomic::Ordering;
use tauri::State;

use launcher_shared::{LogBuffer, LogStats};
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
