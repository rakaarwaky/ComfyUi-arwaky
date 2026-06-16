// PURPOSE: Process state value objects — Tauri-managed state containers for ComfyUI process lifecycle.

use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Whether the webview has been redirected to the ComfyUI server.
pub struct RedirectionState {
    pub is_redirected: AtomicBool,
}

/// Collection of join handles for background threads (writer, stdout, stderr, poll).
pub struct ThreadHandles {
    pub handles: Mutex<Vec<JoinHandle<()>>>,
}

/// Atomic shutdown flag checked by polling threads.
pub struct ShutdownSignal {
    pub shutdown: AtomicBool,
}

/// The spawned python Child process, held for lifecycle management.
pub struct ComfyUiState {
    pub child: Mutex<Option<Child>>,
}
