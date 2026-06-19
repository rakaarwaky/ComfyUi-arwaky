// PURPOSE: log-engine — logging free functions + surface commands for log polling.

mod file_log_writer;
mod infrastructure_gpu_monitor_adapter;
mod surface_log_commands;
mod surface_log_writer;

pub use file_log_writer::*;
pub use infrastructure_gpu_monitor_adapter::*;
pub use surface_log_commands::*;
pub use surface_log_writer::*;

// ── Constants ──

pub const MAX_LOG_ENTRIES: usize = 2000;
pub const LOG_CHANNEL_CAPACITY: usize = 1000;
pub const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
pub const BATCH_MAX_SIZE: usize = 50;

/// Wrapper for app start time — managed by Tauri for uptime calculation.
pub struct StartTime(pub std::time::Instant);
