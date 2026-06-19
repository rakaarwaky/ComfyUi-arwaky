// PURPOSE: Domain event types for backend installation lifecycle.

use crate::BackendInstallError;
use serde::Serialize;

#[derive(Debug, Serialize, serde::Deserialize)]
pub enum BackendInstallEvent {
    Downloading {
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    Verifying,
    Extracting {
        files_extracted: u64,
    },
    Complete,
    Failed(BackendInstallError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_downloading() {
        let e = BackendInstallEvent::Downloading {
            bytes_downloaded: 100,
            total_bytes: 500,
        };
        let s = format!("{:?}", e);
        assert!(s.contains("100") && s.contains("500"));
    }
    #[test]
    fn debug_verifying() {
        assert!(format!("{:?}", BackendInstallEvent::Verifying).contains("Verifying"));
    }
    #[test]
    fn debug_extracting() {
        let e = BackendInstallEvent::Extracting {
            files_extracted: 42,
        };
        assert!(format!("{:?}", e).contains("42"));
    }
    #[test]
    fn debug_complete() {
        assert!(format!("{:?}", BackendInstallEvent::Complete).contains("Complete"));
    }
    #[test]
    fn debug_failed() {
        let e = BackendInstallEvent::Failed(BackendInstallError::DownloadCancelled);
        assert!(format!("{:?}", e).contains("Failed") || format!("{:?}", e).contains("cancelled"));
    }
    #[test]
    fn serde_roundtrip_downloading() {
        let e = BackendInstallEvent::Downloading {
            bytes_downloaded: 100,
            total_bytes: 500,
        };
        let j = serde_json::to_string(&e).unwrap();
        let d: BackendInstallEvent = serde_json::from_str(&j).unwrap();
        assert!(matches!(
            d,
            BackendInstallEvent::Downloading {
                bytes_downloaded: 100,
                total_bytes: 500
            }
        ));
    }
    #[test]
    fn serde_roundtrip_complete() {
        let j = serde_json::to_string(&BackendInstallEvent::Complete).unwrap();
        let d: BackendInstallEvent = serde_json::from_str(&j).unwrap();
        assert!(matches!(d, BackendInstallEvent::Complete));
    }
    #[test]
    fn serde_roundtrip_failed() {
        let e = BackendInstallEvent::Failed(BackendInstallError::DownloadCancelled);
        let j = serde_json::to_string(&e).unwrap();
        let d: BackendInstallEvent = serde_json::from_str(&j).unwrap();
        assert!(matches!(d, BackendInstallEvent::Failed(_)));
    }
}
