// PURPOSE: Health state value object — system health snapshot for frontend polling.

use crate::taxonomy_gpu_metrics_vo::GpuMetrics;

/// Health state of the launcher and backend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthState {
    pub backend_alive: bool,
    pub uptime_secs: u64,
    pub gpu: GpuMetrics,
    pub logs_received: u64,
    pub logs_dropped: u64,
    pub backend_pid: Option<u32>,
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            backend_alive: false,
            uptime_secs: 0,
            gpu: GpuMetrics::unknown(),
            logs_received: 0,
            logs_dropped: 0,
            backend_pid: None,
        }
    }
}

impl Default for HealthState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.backend_alive { "UP" } else { "DOWN" };
        write!(
            f,
            "Backend:{} uptime={}s logs={}/{} GPU:{}",
            status,
            self.uptime_secs,
            self.logs_received,
            self.logs_dropped,
            self.gpu,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_health_is_down() {
        let h = HealthState::new();
        assert!(!h.backend_alive);
        assert_eq!(h.uptime_secs, 0);
    }

    #[test]
    fn default_is_new() {
        let h = HealthState::default();
        assert!(!h.backend_alive);
    }

    #[test]
    fn display_down() {
        let h = HealthState::new();
        assert!(h.to_string().contains("DOWN"));
    }

    #[test]
    fn display_up() {
        let mut h = HealthState::new();
        h.backend_alive = true;
        assert!(h.to_string().contains("UP"));
    }

    #[test]
    fn serde_roundtrip() {
        let h = HealthState::new();
        let json = serde_json::to_string(&h).unwrap();
        let back: HealthState = serde_json::from_str(&json).unwrap();
        assert!(!back.backend_alive);
    }
}
