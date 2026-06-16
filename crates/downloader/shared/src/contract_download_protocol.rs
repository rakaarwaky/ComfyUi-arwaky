use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::taxonomy_config_vo::Config;
use crate::taxonomy_download_event_vo::DownloadEvent;
use crate::taxonomy_model_vo::Model;

/// Protocol for download capability — implemented by capabilities layer.
pub trait DownloadProtocol: Send + Sync {
    /// Simple file download (url to dest path).
    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String>;

    /// Full download with HEAD probe, parallel/single, SHA256, progress reporting.
    /// Used by orchestrator's coordinator thread.
    fn download_one_model(
        &self,
        worker_id: usize,
        model: &Model,
        config: &Config,
        cancel_token: &Arc<AtomicBool>,
        tx: &Sender<DownloadEvent>,
    ) -> Result<(), String>;
}
