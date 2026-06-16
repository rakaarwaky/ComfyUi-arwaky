// PURPOSE: process-manager — ProcessPort impl + surface command for process lifecycle.

mod infrastructure_process_spawner;

pub use infrastructure_process_spawner::ProcessSpawner;

// ── Constants ──

pub const PORT_POLL_TIMEOUT_SECS: u32 = 60;
pub const PORT_POLL_INTERVAL_MS: u64 = 1000;
