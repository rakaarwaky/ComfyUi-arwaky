// PURPOSE: GPU metrics value object — runtime GPU state snapshot.

/// Runtime GPU metrics collected by periodic polling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuMetrics {
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub vram_free_bytes: u64,
    pub gpu_utilization_pct: u32,
    pub temperature_celsius: u32,
    pub clock_mhz: u32,
    pub power_watts: f32,
}

impl GpuMetrics {
    pub fn unknown() -> Self {
        Self {
            vram_total_bytes: 0,
            vram_used_bytes: 0,
            vram_free_bytes: 0,
            gpu_utilization_pct: 0,
            temperature_celsius: 0,
            clock_mhz: 0,
            power_watts: 0.0,
        }
    }

    pub fn vram_usage_pct(&self) -> f32 {
        if self.vram_total_bytes == 0 {
            return 0.0;
        }
        (self.vram_used_bytes as f32 / self.vram_total_bytes as f32) * 100.0
    }
}

impl std::fmt::Display for GpuMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VRAM {}/{}MB ({}%) | GPU {}% | {}°C | {}MHz | {:.1}W",
            self.vram_used_bytes / (1024 * 1024),
            self.vram_total_bytes / (1024 * 1024),
            self.vram_usage_pct() as u32,
            self.gpu_utilization_pct,
            self.temperature_celsius,
            self.clock_mhz,
            self.power_watts,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_defaults() {
        let m = GpuMetrics::unknown();
        assert_eq!(m.vram_total_bytes, 0);
        assert_eq!(m.gpu_utilization_pct, 0);
    }

    #[test]
    fn vram_usage_pct_calculation() {
        let m = GpuMetrics {
            vram_total_bytes: 16_000_000_000,
            vram_used_bytes: 8_000_000_000,
            vram_free_bytes: 8_000_000_000,
            gpu_utilization_pct: 50,
            temperature_celsius: 65,
            clock_mhz: 2100,
            power_watts: 150.0,
        };
        assert!((m.vram_usage_pct() - 50.0).abs() < 0.1);
    }

    #[test]
    fn vram_usage_pct_zero_total() {
        let m = GpuMetrics::unknown();
        assert!((m.vram_usage_pct() - 0.0).abs() < 0.01);
    }

    #[test]
    fn display_format() {
        let m = GpuMetrics {
            vram_total_bytes: 16_000_000_000,
            vram_used_bytes: 8_000_000_000,
            vram_free_bytes: 8_000_000_000,
            gpu_utilization_pct: 50,
            temperature_celsius: 65,
            clock_mhz: 2100,
            power_watts: 150.0,
        };
        let s = m.to_string();
        assert!(s.contains("50%"));
        assert!(s.contains("65°"));
    }

    #[test]
    fn serde_roundtrip() {
        let m = GpuMetrics::unknown();
        let json = serde_json::to_string(&m).unwrap();
        let back: GpuMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(back.vram_total_bytes, 0);
    }
}
