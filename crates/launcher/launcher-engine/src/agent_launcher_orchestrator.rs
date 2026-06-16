// PURPOSE: launcher-engine — agent orchestrator. Imports ONLY contracts from shared.
// Holds Arc<dyn Port> + Arc<dyn Protocol>, implements LauncherAggregate.

use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, atomic::AtomicBool};

use launcher_shared::contract_config_port::ConfigPort;
use launcher_shared::contract_backend_install_protocol::BackendInstallProtocol;
use launcher_shared::contract_gpu_detection_protocol::GpuDetectionProtocol;
use launcher_shared::contract_process_port::{ProcessPort, SpawnParams};
use launcher_shared::contract_launcher_aggregate::LauncherAggregate;
use launcher_shared::BackendStatus;
use launcher_shared::{
    AppConfig, BackendInstallError, BackendInstallEvent, ConfigError, GpuIndex, InstallDir,
    LogMessage,
};

pub struct LauncherOrchestrator {
    config_port: Arc<dyn ConfigPort>,
    backend_protocol: Arc<dyn BackendInstallProtocol>,
    gpu_protocol: Arc<dyn GpuDetectionProtocol>,
    process_port: Arc<dyn ProcessPort>,
}

impl LauncherOrchestrator {
    pub fn new(
        config_port: Arc<dyn ConfigPort>,
        backend_protocol: Arc<dyn BackendInstallProtocol>,
        gpu_protocol: Arc<dyn GpuDetectionProtocol>,
        process_port: Arc<dyn ProcessPort>,
    ) -> Self {
        Self {
            config_port,
            backend_protocol,
            gpu_protocol,
            process_port,
        }
    }
}

impl LauncherAggregate for LauncherOrchestrator {
    fn check_backend_status(&self) -> BackendStatus {
        match self.config_port.load() {
            Ok(config) => {
                let is_custom = config
                    .python_path
                    .as_ref()
                    .and_then(|py| {
                        config.comfyui_dir.as_ref().map(|comfy| {
                            std::path::Path::new(py).exists() && std::path::Path::new(comfy).exists()
                        })
                    })
                    .unwrap_or(false);

                if is_custom {
                    return BackendStatus::CustomInstall;
                }

                if let Some(install_dir) = self.backend_protocol.default_install_dir() {
                    if self.backend_protocol.is_installed(&install_dir) {
                        return BackendStatus::Installed {
                            version: self.backend_protocol.installed_version(&install_dir),
                        };
                    }
                }
                BackendStatus::NotInstalled
            }
            Err(_) => BackendStatus::NotInstalled,
        }
    }

    fn backend_install_dir(&self) -> InstallDir {
        self.backend_protocol
            .default_install_dir()
            .unwrap_or_else(|| InstallDir(PathBuf::from(".")))
    }

    fn read_config(&self) -> Result<AppConfig, ConfigError> {
        self.config_port.load()
    }

    fn ensure_config(&self, app_config_dir: &InstallDir) -> AppConfig {
        self.config_port.ensure(app_config_dir).unwrap_or_default()
    }

    fn start_backend_download(
        &self,
        cancel: Arc<AtomicBool>,
        on_event: Box<dyn Fn(BackendInstallEvent) + Send>,
        on_complete: Box<dyn Fn() + Send>,
        on_error: Box<dyn Fn(BackendInstallError) + Send>,
    ) {
        let install_dir = self.backend_install_dir();
        let archive_url = self.backend_protocol.backend_download_url();
        let backend = Arc::clone(&self.backend_protocol);

        std::thread::spawn(move || {
            let expected_sha = backend.expected_sha256();
            let result = backend.install(
                &install_dir,
                &archive_url,
                expected_sha.as_ref(),
                &cancel,
                &|event| on_event(event),
            );
            match result {
                Ok(()) => on_complete(),
                Err(e) => on_error(e),
            }
        });
    }

    fn cancel_backend_download(&self, cancel: &AtomicBool) {
        cancel.store(true, std::sync::atomic::Ordering::Release);
    }

    fn start_comfyui(
        &self,
        log_tx: SyncSender<LogMessage>,
        on_exit: Box<dyn Fn() + Send>,
    ) -> Result<(), BackendInstallError> {
        let config = self.config_port.load().map_err(|e| {
            BackendInstallError::VerificationFailed(format!("Config load: {}", e))
        })?;

        // Resolve paths
        let default_dir = self.backend_protocol.default_install_dir();
        let python_path = launcher_shared::resolve_python_path(
            config.python_path.as_deref(),
            default_dir.as_ref().map(|d| d.as_path()),
        );
        let comfyui_dir = launcher_shared::resolve_comfyui_dir(
            config.comfyui_dir.as_deref(),
            default_dir.as_ref().map(|d| d.as_path()),
        );
        let extra_model_paths = launcher_shared::resolve_extra_model_paths(
            config.extra_model_paths.as_deref(),
            default_dir.as_ref().map(|d| d.as_path()),
        );

        let output_dir = config.output_dir.as_ref().map(std::path::PathBuf::from);
        let input_dir = config.input_dir.as_ref().map(std::path::PathBuf::from);
        let user_dir = config.user_dir.as_ref().map(std::path::PathBuf::from);

        let gpu_index = self.gpu_protocol.detect_dgpu_index()
            .unwrap_or_else(|_| GpuIndex("0".to_string()));
        let hsa_override = self.gpu_protocol.detect_hsa_override()
            .unwrap_or(None);

        self.process_port.spawn(SpawnParams {
            python_path: &python_path,
            comfyui_dir: &comfyui_dir,
            extra_model_paths: &extra_model_paths,
            gpu_index: &gpu_index,
            hsa_override: hsa_override.as_ref(),
            output_dir: output_dir.as_deref(),
            input_dir: input_dir.as_deref(),
            user_dir: user_dir.as_deref(),
            log_tx,
            on_exit,
        })
        .map_err(|e| BackendInstallError::VerificationFailed(format!("Process: {e}")))
    }
}
