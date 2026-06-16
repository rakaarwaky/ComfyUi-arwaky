// PURPOSE: log-engine — logging free functions + surface commands for log polling.

mod surface_log_writer;
mod surface_log_commands;

pub use surface_log_writer::*;
pub use surface_log_commands::*;

// ── Constants ──

pub const MAX_LOG_ENTRIES: usize = 2000;
pub const LOG_CHANNEL_CAPACITY: usize = 1000;
pub const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
pub const BATCH_MAX_SIZE: usize = 50;
