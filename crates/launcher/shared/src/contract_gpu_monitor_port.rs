// PURPOSE: Port for runtime GPU monitoring — periodic metrics collection.

use crate::taxonomy_gpu_metrics_vo::GpuMetrics;

/// Port for querying current GPU metrics.
/// Implementations live in infrastructure (rocm-smi adapter).
pub trait GpuMonitorPort: Send + Sync {
    /// Get the latest GPU metrics snapshot.
    fn get_metrics(&self) -> GpuMetrics;
}
