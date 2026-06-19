// PURPOSE: Domain error types for GPU detection.

use std::fmt;

#[derive(Debug)]
pub enum GpuError {
    RocmNotAvailable,
    NoGpuFound,
    HsaDetectionFailed(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RocmNotAvailable => write!(f, "ROCm is not available on this system"),
            Self::NoGpuFound => write!(f, "No compatible GPU found"),
            Self::HsaDetectionFailed(msg) => write!(f, "HSA detection failed: {msg}"),
        }
    }
}

impl std::error::Error for GpuError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_rocm_not_available() {
        assert_eq!(
            GpuError::RocmNotAvailable.to_string(),
            "ROCm is not available on this system"
        );
    }
    #[test]
    fn display_no_gpu_found() {
        assert_eq!(GpuError::NoGpuFound.to_string(), "No compatible GPU found");
    }
    #[test]
    fn display_hsa_detection_failed() {
        assert_eq!(
            GpuError::HsaDetectionFailed("x".into()).to_string(),
            "HSA detection failed: x"
        );
    }
}
