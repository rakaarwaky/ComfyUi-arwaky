use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

pub const MAX_LOG_ENTRIES: usize = 2000;
pub const LOG_CHANNEL_CAPACITY: usize = 1000;
pub const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
pub const BATCH_MAX_SIZE: usize = 50;

pub enum LogMessage {
    Stdout(String),
    Stderr(String),
    Launcher(String),
}

pub struct LogBuffer {
    pub logs: Mutex<VecDeque<(u64, String)>>,
    pub next_id: AtomicU64,
}

pub struct LogSender {
    pub tx: Mutex<Option<SyncSender<LogMessage>>>,
}

pub struct LogStats {
    pub dropped: AtomicU64,
    pub total_received: AtomicU64,
}

pub fn log_info(app_handle: &tauri::AppHandle, message: &str) {
    if let Some(sender) = app_handle.try_state::<LogSender>() {
        if let Ok(tx_guard) = sender.tx.lock() {
            if let Some(ref tx) = *tx_guard {
                if tx
                    .try_send(LogMessage::Launcher(message.to_string()))
                    .is_err()
                {
                    eprintln!("[Launcher] {}", message);
                }
            }
        }
    } else {
        println!("[Launcher] {}", message);
    }
}

pub fn flush_batch(app_handle: &tauri::AppHandle, batch: &mut Vec<(&'static str, String)>) {
    if batch.is_empty() {
        return;
    }
    let mut grouped: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    for (event, msg) in batch.drain(..) {
        grouped.entry(event).or_default().push(msg);
    }
    for (event, messages) in grouped {
        let _ = app_handle.emit(event, messages);
    }
}

#[tauri::command]
pub fn get_logs(
    log_buffer: tauri::State<'_, LogBuffer>,
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

#[tauri::command]
pub fn get_log_stats(stats: tauri::State<'_, LogStats>) -> (u64, u64) {
    (
        stats.total_received.load(Ordering::Relaxed),
        stats.dropped.load(Ordering::Relaxed),
    )
}
