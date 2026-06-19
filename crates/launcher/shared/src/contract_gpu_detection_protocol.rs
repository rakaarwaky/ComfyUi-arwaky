// PURPOSE: Protocol for AMD ROCm GPU / HSA detection capability.

use crate::taxonomy_gpu_error_vo::GpuError;
use crate::taxonomy_gpu_index_vo::GpuIndex;
use crate::taxonomy_hsa_override_vo::HsaOverride;

pub trait GpuDetectionProtocol: Send + Sync {
    fn detect_dgpu_index(&self) -> Result<GpuIndex, GpuError>;
    fn detect_hsa_override(&self) -> Result<Option<HsaOverride>, GpuError>;
}
