use std::path::Path;

/// Port for file system validation operations.
pub trait FileValidationPort: Send + Sync {
    fn file_exists_valid(&self, path: &Path, expected_size: u64, url: Option<&str>) -> bool;
    fn verify_sha256(&self, path: &Path, expected_hex: &str) -> bool;
    fn sanitize_filename(&self, filename: &str) -> String;
    fn get_available_space(&self, path: &Path) -> std::io::Result<u64>;
}
