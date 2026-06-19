// PURPOSE: log-engine — logging free functions + surface commands for log polling.

mod infrastructure_file_log_writer;
mod infrastructure_gpu_monitor_adapter;
mod infrastructure_log_writer;
mod surface_log_command;

pub use infrastructure_file_log_writer::FileLogWriter;
pub use infrastructure_gpu_monitor_adapter::{GpuMetricsAtomic, GpuMonitorAdapter};
pub use infrastructure_log_writer::LogEmitter;
pub use surface_log_command::{
    get_gpu_metrics, get_health, get_log_stats, get_logs, open_log_viewer,
};

// ── Constants ──

pub const MAX_LOG_ENTRIES: usize = 2000;
pub const LOG_CHANNEL_CAPACITY: usize = 1000;
pub const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
pub const BATCH_MAX_SIZE: usize = 50;

/// Wrapper for app start time — managed by Tauri for uptime calculation.
pub struct StartTime(pub std::time::Instant);
