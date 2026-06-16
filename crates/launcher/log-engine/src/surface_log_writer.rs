// PURPOSE: Log writer — free functions for log writing and batch emitting.

use std::collections::HashMap;
use tauri::{Emitter, Manager};

use launcher_shared::LogMessage;

use launcher_shared::LogSender;

/// Send a launcher info message to the log channel.
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

/// Flush a batch of (event_name, message) pairs to the frontend via Tauri emit.
pub fn flush_batch(app_handle: &tauri::AppHandle, batch: &mut Vec<(&'static str, String)>) {
    if batch.is_empty() {
        return;
    }
    let mut grouped: HashMap<&'static str, Vec<String>> = HashMap::new();
    for (event, msg) in batch.drain(..) {
        grouped.entry(event).or_default().push(msg);
    }
    for (event, messages) in grouped {
        let _ = app_handle.emit(event, messages);
    }
}
