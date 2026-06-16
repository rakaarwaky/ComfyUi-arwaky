// PURPOSE: Log state value objects — Tauri-managed state containers for log buffer/channel/stats.

use std::collections::VecDeque;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

use crate::taxonomy_log_message_vo::LogMessage;

/// Ring buffer of (id, formatted_message) pairs for frontend polling.
pub struct LogBuffer {
    pub logs: Mutex<VecDeque<(u64, String)>>,
    pub next_id: AtomicU64,
}

/// MPSC sender end for log messages (tx stored for stdout/stderr reader threads).
pub struct LogSender {
    pub tx: Mutex<Option<SyncSender<LogMessage>>>,
}

/// Atomic counters for log volume tracking.
pub struct LogStats {
    pub dropped: AtomicU64,
    pub total_received: AtomicU64,
}
