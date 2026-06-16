// PURPOSE: Port for spawning the ComfyUI Python process with lifecycle management.

use std::path::Path;
use std::sync::mpsc::SyncSender;

use crate::taxonomy_gpu_index_vo::GpuIndex;
use crate::taxonomy_hsa_override_vo::HsaOverride;
use crate::taxonomy_log_message_vo::LogMessage;
use crate::taxonomy_process_error_vo::ProcessError;

/// Bundled parameters for ProcessPort::spawn to avoid too_many_arguments lint.
pub struct SpawnParams<'a> {
    pub python_path: &'a Path,
    pub comfyui_dir: &'a Path,
    pub extra_model_paths: &'a Path,
    pub gpu_index: &'a GpuIndex,
    pub hsa_override: Option<&'a HsaOverride>,
    pub output_dir: Option<&'a Path>,
    pub input_dir: Option<&'a Path>,
    pub user_dir: Option<&'a Path>,
    pub log_tx: SyncSender<LogMessage>,
    pub on_exit: Box<dyn Fn() + Send>,
}

pub trait ProcessPort: Send + Sync {
    /// Spawns ComfyUI process with built-in I/O handling.
    /// Reads stdout/stderr and sends to log_tx.
    /// Calls on_exit when process exits or panics.
    fn spawn(&self, params: SpawnParams) -> Result<(), ProcessError>;
}
