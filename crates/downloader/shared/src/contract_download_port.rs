use std::path::Path;

/// Port for downloading a model file.
pub trait DownloadPort: Send + Sync {
    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String>;
}
