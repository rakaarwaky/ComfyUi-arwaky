use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::taxonomy_config_vo::Config;
use crate::taxonomy_download_event_vo::DownloadEvent;
use crate::taxonomy_model_vo::Model;

/// Aggregate trait — the ONE interface surfaces call.
/// Orchestrator implements this. Surfaces call ONLY this, never ports/protocols directly.
pub trait DownloaderAggregate: Send + Sync {
    fn get_models(&self) -> Vec<Model>;
    fn get_config(&self) -> Config;
    fn file_exists_valid(&self, path: &Path, expected: u64, url: Option<&str>) -> bool;
    fn get_cached_size(&self, url: &str) -> Option<u64>;
    fn set_cached_size(&self, url: &str, size: u64);
    fn save_cache(&self);
    fn get_available_space(&self, path: &Path) -> u64;

    /// Spawn coordinator thread, return event receiver.
    fn start_download_coordinator(
        &self,
        selected: Vec<(usize, Model)>,
        config: Config,
        cancel: Arc<AtomicBool>,
    ) -> Receiver<DownloadEvent>;

    /// Spawn metadata refresh thread, return event receiver.
    fn refresh_metadata(
        &self,
        models: Vec<(usize, Model)>,
        config: Config,
    ) -> Receiver<DownloadEvent>;
}
