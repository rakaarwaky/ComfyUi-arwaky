// PURPOSE: Domain error types for backend installation.

use std::fmt;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum BackendInstallError {
    DiskSpaceLow { available: u64, needed: u64 },
    DownloadFailed { http_status: u16 },
    ConnectionFailed(String),
    ChecksumMismatch { expected: String, computed: String },
    ExtractionFailed(String),
    DownloadCancelled,
    ExtractionCancelled,
    IoError(String),
    VerificationFailed(String),
}

impl fmt::Display for BackendInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiskSpaceLow { available, needed } => {
                write!(f, "Insufficient disk space: {available} GB available, need at least {needed} GB")
            }
            Self::DownloadFailed { http_status } => {
                write!(f, "Download failed: server returned HTTP {http_status}")
            }
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::ChecksumMismatch { expected, computed } => {
                write!(f, "Checksum mismatch: expected {expected}, got {computed}")
            }
            Self::ExtractionFailed(msg) => write!(f, "Extraction failed: {msg}"),
            Self::DownloadCancelled => write!(f, "Download cancelled"),
            Self::ExtractionCancelled => write!(f, "Extraction cancelled"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::VerificationFailed(msg) => write!(f, "Verification failed: {msg}"),
        }
    }
}

impl std::error::Error for BackendInstallError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_disk_space_low() {
        let err = BackendInstallError::DiskSpaceLow { available: 5, needed: 20 };
        assert_eq!(err.to_string(), "Insufficient disk space: 5 GB available, need at least 20 GB");
    }

    #[test]
    fn display_download_failed() {
        let err = BackendInstallError::DownloadFailed { http_status: 404 };
        assert_eq!(err.to_string(), "Download failed: server returned HTTP 404");
    }

    #[test]
    fn display_connection_failed() {
        let err = BackendInstallError::ConnectionFailed("timeout".into());
        assert_eq!(err.to_string(), "Connection failed: timeout");
    }

    #[test]
    fn display_checksum_mismatch() {
        let err = BackendInstallError::ChecksumMismatch {
            expected: "abc".into(),
            computed: "def".into(),
        };
        assert_eq!(err.to_string(), "Checksum mismatch: expected abc, got def");
    }

    #[test]
    fn display_extraction_failed() {
        let err = BackendInstallError::ExtractionFailed("corrupt archive".into());
        assert_eq!(err.to_string(), "Extraction failed: corrupt archive");
    }

    #[test]
    fn display_download_cancelled() {
        let err = BackendInstallError::DownloadCancelled;
        assert_eq!(err.to_string(), "Download cancelled");
    }

    #[test]
    fn display_extraction_cancelled() {
        let err = BackendInstallError::ExtractionCancelled;
        assert_eq!(err.to_string(), "Extraction cancelled");
    }

    #[test]
    fn display_io_error() {
        let err = BackendInstallError::IoError("permission denied".into());
        assert_eq!(err.to_string(), "I/O error: permission denied");
    }

    #[test]
    fn display_verification_failed() {
        let err = BackendInstallError::VerificationFailed("python not found".into());
        assert_eq!(err.to_string(), "Verification failed: python not found");
    }
}
