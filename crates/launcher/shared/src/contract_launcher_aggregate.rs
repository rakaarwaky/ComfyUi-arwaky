// PURPOSE: Aggregate trait — the ONE interface surfaces call.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use crate::taxonomy_app_config_vo::AppConfig;
use crate::taxonomy_backend_install_error_vo::BackendInstallError;
use crate::taxonomy_backend_install_event_vo::BackendInstallEvent;
use crate::taxonomy_backend_status_vo::BackendStatus;
use crate::taxonomy_install_dir_vo::InstallDir;
use crate::taxonomy_log_message_vo::LogMessage;
use crate::ConfigError;

pub trait LauncherAggregate: Send + Sync {
    fn check_backend_status(&self) -> BackendStatus;
    fn backend_install_dir(&self) -> InstallDir;
    fn ensure_config(&self, app_config_dir: &InstallDir) -> AppConfig;
    fn read_config(&self) -> Result<AppConfig, ConfigError>;

    fn start_backend_download(
        &self,
        cancel: Arc<AtomicBool>,
        on_event: Box<dyn Fn(BackendInstallEvent) + Send>,
        on_complete: Box<dyn Fn() + Send>,
        on_error: Box<dyn Fn(BackendInstallError) + Send>,
    );
    fn cancel_backend_download(&self, cancel: &AtomicBool);

    fn start_comfyui(
        &self,
        log_tx: SyncSender<LogMessage>,
        on_exit: Box<dyn Fn() + Send>,
    ) -> Result<(), BackendInstallError>;
}
