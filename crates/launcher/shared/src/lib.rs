// PURPOSE: shared — taxonomy value objects + contract ports/protocols/aggregate.

pub mod contract_backend_install_protocol;
pub mod contract_config_port;
pub mod contract_gpu_detection_protocol;
pub mod contract_gpu_monitor_port;
pub mod contract_launcher_aggregate;
pub mod contract_process_port;
pub mod taxonomy_app_config_vo;
pub mod taxonomy_archive_url_vo;
pub mod taxonomy_backend_install_error_vo;
pub mod taxonomy_backend_install_event_vo;
pub mod taxonomy_backend_status_vo;
pub mod taxonomy_config_error_vo;
pub mod taxonomy_gpu_error_vo;
pub mod taxonomy_gpu_index_vo;
pub mod taxonomy_gpu_metrics_vo;
pub mod taxonomy_hsa_override_vo;
pub mod taxonomy_health_state_vo;
pub mod taxonomy_install_dir_vo;
pub mod taxonomy_log_level_vo;
pub mod taxonomy_log_message_vo;
pub mod taxonomy_log_state_vo;
pub mod taxonomy_path_utils;
pub mod taxonomy_process_error_vo;
pub mod taxonomy_process_state_vo;
pub mod taxonomy_sha256_vo;

pub use taxonomy_app_config_vo::AppConfig;
pub use taxonomy_archive_url_vo::ArchiveUrl;
pub use taxonomy_backend_install_error_vo::BackendInstallError;
pub use taxonomy_backend_install_event_vo::BackendInstallEvent;
pub use taxonomy_backend_status_vo::BackendStatus;
pub use taxonomy_config_error_vo::ConfigError;
pub use taxonomy_gpu_error_vo::GpuError;
pub use taxonomy_gpu_index_vo::GpuIndex;
pub use taxonomy_gpu_metrics_vo::GpuMetrics;
pub use taxonomy_hsa_override_vo::HsaOverride;
pub use taxonomy_health_state_vo::HealthState;
pub use taxonomy_install_dir_vo::InstallDir;
pub use taxonomy_log_level_vo::LogLevel;
pub use taxonomy_log_message_vo::LogMessage;
pub use taxonomy_log_state_vo::{LogBuffer, LogSender, LogStats};
pub use taxonomy_path_utils::{
    normalize_path, resolve_comfyui_dir, resolve_extra_model_paths, resolve_path,
    resolve_python_path,
};
pub use taxonomy_process_error_vo::ProcessError;
pub use taxonomy_process_state_vo::{
    ComfyUiState, RedirectionState, ShutdownSignal, ThreadHandles,
};
pub use taxonomy_sha256_vo::Sha256Hash;
