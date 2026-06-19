// PURPOSE: GPU monitor adapter — periodic rocm-smi polling for runtime metrics.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use launcher_shared::contract_gpu_monitor_port::GpuMonitorPort;
use launcher_shared::GpuMetrics;

const POLL_INTERVAL_SECS: u64 = 2;

fn get_rocm_path() -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
    paths.insert(0, std::path::PathBuf::from("/opt/rocm/bin"));
    paths.insert(0, std::path::PathBuf::from("/opt/rocm-7.2.4/bin"));
    std::env::join_paths(paths).unwrap_or(path)
}

/// Shared atomic GPU metrics for lock-free reads from any thread.
pub struct GpuMetricsAtomic {
    pub vram_total: AtomicU64,
    pub vram_used: AtomicU64,
    pub vram_free: AtomicU64,
    pub utilization: AtomicU32,
    pub temperature: AtomicU32,
    pub clock_mhz: AtomicU32,
    pub power_watts: AtomicU32,
    pub active: AtomicBool,
}

impl GpuMetricsAtomic {
    pub fn new() -> Self {
        Self {
            vram_total: AtomicU64::new(0),
            vram_used: AtomicU64::new(0),
            vram_free: AtomicU64::new(0),
            utilization: AtomicU32::new(0),
            temperature: AtomicU32::new(0),
            clock_mhz: AtomicU32::new(0),
            power_watts: AtomicU32::new(0),
            active: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> GpuMetrics {
        GpuMetrics {
            vram_total_bytes: self.vram_total.load(Ordering::Relaxed),
            vram_used_bytes: self.vram_used.load(Ordering::Relaxed),
            vram_free_bytes: self.vram_free.load(Ordering::Relaxed),
            gpu_utilization_pct: self.utilization.load(Ordering::Relaxed),
            temperature_celsius: self.temperature.load(Ordering::Relaxed),
            clock_mhz: self.clock_mhz.load(Ordering::Relaxed),
            power_watts: self.power_watts.load(Ordering::Relaxed) as f32 / 100.0,
        }
    }
}

impl Default for GpuMetricsAtomic {
    fn default() -> Self {
        Self::new()
    }
}

/// Infrastructure adapter: polls rocm-smi and updates shared atomic metrics.
pub struct GpuMonitorAdapter {
    metrics: Arc<GpuMetricsAtomic>,
}

impl GpuMonitorAdapter {
    pub fn new() -> (Self, Arc<GpuMetricsAtomic>) {
        let metrics = Arc::new(GpuMetricsAtomic::new());
        let adapter = Self {
            metrics: metrics.clone(),
        };
        (adapter, metrics)
    }

    /// Spawn the background polling thread.
    pub fn start_polling(&self) {
        let metrics = self.metrics.clone();
        thread::spawn(move || {
            poll_loop(&metrics);
        });
    }
}

impl GpuMonitorPort for GpuMonitorAdapter {
    fn get_metrics(&self) -> GpuMetrics {
        self.metrics.snapshot()
    }
}

fn poll_loop(metrics: &GpuMetricsAtomic) {
    loop {
        poll_rocm_smi(metrics);
        thread::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

fn poll_rocm_smi(metrics: &GpuMetricsAtomic) {
    let rocm_path = get_rocm_path();

    // VRAM info
    if let Ok(output) = std::process::Command::new("rocm-smi")
        .env("PATH", &rocm_path)
        .arg("--showmeminfo")
        .arg("vram")
        .arg("--json")
        .output()
    {
        if output.status.success() {
            parse_vram_json(&output.stdout, metrics);
        }
    }

    // GPU utilization, temperature, clock, power
    let rocm_path2 = get_rocm_path();
    if let Ok(output) = std::process::Command::new("rocm-smi")
        .env("PATH", &rocm_path2)
        .arg("--showuse")
        .arg("--showtemp")
        .arg("--showclocks")
        .arg("--showpower")
        .arg("--json")
        .output()
    {
        if output.status.success() {
            parse_use_json(&output.stdout, metrics);
        }
    }
}

fn parse_vram_json(stdout: &[u8], metrics: &GpuMetricsAtomic) {
    let json: serde_json::Value = match serde_json::from_slice(stdout) {
        Ok(v) => v,
        Err(_) => return,
    };

    // rocm-smi JSON format: {"card0": {"VRAM Total Memory": ..., "VRAM Total Used Memory": ...}}
    if let Some(card) = json.get("card0") {
        if let Some(total) = card.get("VRAM Total Memory (B)").or_else(|| card.get("VRAM Total Memory")) {
            if let Some(val) = total.as_u64() {
                metrics.vram_total.store(val, Ordering::Relaxed);
            }
        }
        if let Some(used) = card.get("VRAM Total Used Memory (B)").or_else(|| card.get("VRAM Total Used Memory")) {
            if let Some(val) = used.as_u64() {
                metrics.vram_used.store(val, Ordering::Relaxed);
                let total = metrics.vram_total.load(Ordering::Relaxed);
                if total > val {
                    metrics.vram_free.store(total - val, Ordering::Relaxed);
                }
            }
        }
    }
}

fn parse_use_json(stdout: &[u8], metrics: &GpuMetricsAtomic) {
    let json: serde_json::Value = match serde_json::from_slice(stdout) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(card) = json.get("card0") {
        // GPU use percentage
        if let Some(use_pct) = card.get("GPU use (%)").or_else(|| card.get("GPU Use (%)")) {
            if let Some(val) = use_pct.as_f64() {
                metrics.utilization.store(val as u32, Ordering::Relaxed);
            }
        }

        // Temperature
        if let Some(temp) = card.get("Temperature (Sensor edge) (C)").or_else(|| card.get("Temperature (edge) (C)")) {
            if let Some(val) = temp.as_f64() {
                metrics.temperature.store(val as u32, Ordering::Relaxed);
            }
        }

        // Clock speed
        if let Some(clock) = card.get("GPU clocks (MHz)").or_else(|| card.get("Clock Speed (MHz)")) {
            if let Some(val) = clock.as_f64() {
                metrics.clock_mhz.store(val as u32, Ordering::Relaxed);
            }
        }

        // Power
        if let Some(power) = card.get("Average Graphics Package Power (W)").or_else(|| card.get("GPU Power (W)")) {
            if let Some(val) = power.as_f64() {
                metrics.power_watts.store((val * 100.0) as u32, Ordering::Relaxed);
            }
        }

        metrics.active.store(true, Ordering::Relaxed);
    }
}
