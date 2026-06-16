// PURPOSE: downloader-file-utils — capabilities: file validation (implements FileValidationProtocol)
// Delegates actual IO to infrastructure (FsAdapter).

use std::path::Path;

use downloader_shared::contract_file_protocol::FileValidationProtocol;

use crate::infrastructure_fs_adapter;

pub struct FileChecker;

impl FileValidationProtocol for FileChecker {
    fn file_exists_valid(&self, path: &Path, expected_size: u64, url: Option<&str>) -> bool {
        infrastructure_fs_adapter::file_exists_valid(path, expected_size, url)
    }

    fn verify_sha256(&self, path: &Path, expected_hex: &str) -> bool {
        infrastructure_fs_adapter::verify_sha256(path, expected_hex)
    }

    fn sanitize_filename(&self, filename: &str) -> String {
        infrastructure_fs_adapter::sanitize_filename(filename)
    }

    fn get_available_space(&self, path: &Path) -> std::io::Result<u64> {
        infrastructure_fs_adapter::get_available_space(path)
    }
}

// ── Re-exports for backward compat (surface & orchestrator still call free functions) ──
pub use crate::infrastructure_fs_adapter::{
    file_exists_valid, get_available_space, sanitize_filename, verify_sha256,
};
pub use downloader_shared::taxonomy_size_vo::format_size;
