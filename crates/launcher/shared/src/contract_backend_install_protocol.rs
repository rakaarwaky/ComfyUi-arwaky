// PURPOSE: Protocol for backend installation capability.

use std::sync::atomic::AtomicBool;

use crate::taxonomy_archive_url_vo::ArchiveUrl;
use crate::taxonomy_backend_install_error_vo::BackendInstallError;
use crate::taxonomy_backend_install_event_vo::BackendInstallEvent;
use crate::taxonomy_install_dir_vo::InstallDir;
use crate::taxonomy_sha256_vo::Sha256Hash;

pub trait BackendInstallProtocol: Send + Sync {
    fn is_installed(&self, install_dir: &InstallDir) -> bool;
    fn installed_version(&self, install_dir: &InstallDir) -> Option<String>;
    fn default_install_dir(&self) -> Option<InstallDir>;
    fn backend_download_url(&self) -> ArchiveUrl;
    fn expected_sha256(&self) -> Option<Sha256Hash>;
    fn install(
        &self,
        install_dir: &InstallDir,
        archive_url: &ArchiveUrl,
        expected_sha256: Option<&Sha256Hash>,
        cancel_token: &AtomicBool,
        on_event: &dyn Fn(BackendInstallEvent),
    ) -> Result<(), BackendInstallError>;
}
