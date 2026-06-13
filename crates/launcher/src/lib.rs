mod config;
mod downloader;
mod gpu;
mod logging;
mod process;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager};

pub struct BackendState {
    pub install_dir: std::path::PathBuf,
    pub is_ready: AtomicBool,
    pub is_downloading: AtomicBool,
    pub cancel_token: Mutex<Arc<AtomicBool>>,
}

#[tauri::command]
fn check_backend_status(
    app_handle: tauri::AppHandle,
    backend: tauri::State<'_, BackendState>,
) -> (bool, Option<String>) {
    let user_config = config::read_app_config(&app_handle);

    if let (Some(ref py_path), Some(ref comfy_path)) =
        (&user_config.python_path, &user_config.comfyui_dir)
    {
        if std::path::Path::new(py_path).exists() && std::path::Path::new(comfy_path).exists() {
            return (true, Some("custom".to_string()));
        }
    }

    let installer = downloader::BackendInstaller::new(
        backend.install_dir.clone(),
        downloader::backend_download_url(),
        Some(downloader::BACKEND_SHA256.to_string()),
    );
    let is_installed = installer.is_installed();
    let version = installer.installed_version();
    (is_installed, version)
}

#[tauri::command]
fn start_backend_download(
    app_handle: tauri::AppHandle,
    backend: tauri::State<'_, BackendState>,
) -> Result<(), String> {
    if backend.is_downloading.load(Ordering::Acquire) {
        return Err("Download already in progress".to_string());
    }

    backend.is_downloading.store(true, Ordering::Release);

    let cancel_token = Arc::new(AtomicBool::new(false));
    {
        let mut guard = backend
            .cancel_token
            .lock()
            .map_err(|p| format!("cancel_token mutex poisoned: {}", p))?;
        *guard = cancel_token.clone();
    }

    let install_dir = backend.install_dir.clone();
    let archive_url = downloader::backend_download_url();
    let app_handle_clone = app_handle.clone();

    std::thread::spawn(move || {
        let installer = downloader::BackendInstaller::new(install_dir, archive_url, None);

        let result = installer.install(
            |progress| {
                let _ = app_handle_clone.emit("comfyui-download-progress", &progress);
            },
            cancel_token,
        );

        match result {
            Ok(()) => {
                let _ = app_handle_clone.emit("comfyui-download-complete", ());
                if let Some(state) = app_handle_clone.try_state::<BackendState>() {
                    state.is_ready.store(true, Ordering::Release);
                }
            }
            Err(e) => {
                let _ = app_handle_clone.emit("comfyui-download-error", e);
            }
        }

        if let Some(state) = app_handle_clone.try_state::<BackendState>() {
            state.is_downloading.store(false, Ordering::Release);
        }
    });

    Ok(())
}

#[tauri::command]
fn cancel_backend_download(backend: tauri::State<'_, BackendState>) -> Result<(), String> {
    match backend.cancel_token.lock() {
        Ok(guard) => guard.store(true, Ordering::Release),
        Err(poisoned) => poisoned.into_inner().store(true, Ordering::Release),
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    std::env::set_var("WEBKIT_FORCE_COMPOSITING_MODE", "1");
    std::env::set_var("GIO_USE_PROXY_RESOLVER", "dummy");
    std::env::set_var("no_proxy", "*");
    std::env::set_var("NO_PROXY", "*");

    let (log_tx, log_rx) =
        std::sync::mpsc::sync_channel::<logging::LogMessage>(logging::LOG_CHANNEL_CAPACITY);

    let app = tauri::Builder::default()
        .manage(process::ComfyUiState {
            child: Mutex::new(None),
        })
        .manage(logging::LogBuffer {
            logs: Mutex::new(std::collections::VecDeque::new()),
            next_id: std::sync::atomic::AtomicU64::new(0),
        })
        .manage(process::RedirectionState {
            is_redirected: AtomicBool::new(false),
        })
        .manage(logging::LogSender {
            tx: Mutex::new(Some(log_tx)),
        })
        .manage(process::ThreadHandles {
            handles: Mutex::new(Vec::new()),
        })
        .manage(process::ShutdownSignal {
            shutdown: AtomicBool::new(false),
        })
        .manage(logging::LogStats {
            dropped: std::sync::atomic::AtomicU64::new(0),
            total_received: std::sync::atomic::AtomicU64::new(0),
        })
        .manage(BackendState {
            install_dir: downloader::default_install_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
            is_ready: AtomicBool::new(false),
            is_downloading: AtomicBool::new(false),
            cancel_token: Mutex::new(Arc::new(AtomicBool::new(false))),
        })
        .invoke_handler(tauri::generate_handler![
            logging::get_logs,
            logging::get_log_stats,
            check_backend_status,
            start_backend_download,
            cancel_backend_download,
            process::start_comfyui,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let app_handle = app.handle().clone();

            config::ensure_user_config(&app_handle);

            // Writer thread: receives MPSC messages, formats, stores, and emits batched to frontend
            let app_handle_writer = app_handle.clone();
            let writer_handle = std::thread::spawn(move || {
                let log_buffer = app_handle_writer.state::<logging::LogBuffer>();
                let redirect_state = app_handle_writer.state::<process::RedirectionState>();

                let mut batch: Vec<(&'static str, String)> =
                    Vec::with_capacity(logging::BATCH_MAX_SIZE);

                loop {
                    match log_rx
                        .recv_timeout(Duration::from_millis(logging::BATCH_FLUSH_INTERVAL_MS))
                    {
                        Ok(msg) => {
                            let is_redirected =
                                redirect_state.is_redirected.load(Ordering::Acquire);

                            if is_redirected {
                                continue;
                            }

                            let (formatted, _is_stderr) = match msg {
                                logging::LogMessage::Stdout(ref line) => {
                                    (format!("[stdout] {}", line), false)
                                }
                                logging::LogMessage::Stderr(ref line) => {
                                    (format!("[stderr] {}", line), true)
                                }
                                logging::LogMessage::Launcher(ref line) => {
                                    (format!("[Launcher] {}", line), false)
                                }
                            };

                            #[cfg(debug_assertions)]
                            if _is_stderr {
                                eprintln!("{}", formatted);
                            } else {
                                println!("{}", formatted);
                            }

                            let id = log_buffer.next_id.fetch_add(1, Ordering::Relaxed);
                            let mut logs = match log_buffer.logs.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            if logs.len() >= logging::MAX_LOG_ENTRIES {
                                logs.pop_front();
                            }
                            logs.push_back((id, formatted.clone()));
                            drop(logs);

                            let event_name: &'static str = match msg {
                                logging::LogMessage::Stdout(_) => "comfyui-log-stdout",
                                logging::LogMessage::Stderr(_) => "comfyui-log-stderr",
                                logging::LogMessage::Launcher(_) => "comfyui-log",
                            };
                            batch.push((event_name, formatted));

                            if batch.len() >= logging::BATCH_MAX_SIZE {
                                logging::flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                logging::flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }
                }

                logging::flush_batch(&app_handle_writer, &mut batch);
            });

            if let Ok(mut handles) = app_handle.state::<process::ThreadHandles>().handles.lock() {
                handles.push(writer_handle);
            }

            logging::log_info(&app_handle, "Starting ComfyUI Desktop Launcher...");

            let user_config = config::read_app_config(&app_handle);
            let is_custom = if let (Some(ref py_path), Some(ref comfy_path)) =
                (&user_config.python_path, &user_config.comfyui_dir)
            {
                std::path::Path::new(py_path).exists() && std::path::Path::new(comfy_path).exists()
            } else {
                false
            };

            let (is_installed, version) = if is_custom {
                (true, Some("custom".to_string()))
            } else {
                let backend_state = app.state::<BackendState>();
                let install_dir = backend_state.install_dir.clone();
                let installer = downloader::BackendInstaller::new(
                    install_dir,
                    downloader::backend_download_url(),
                    Some(downloader::BACKEND_SHA256.to_string()),
                );
                (installer.is_installed(), installer.installed_version())
            };

            if is_installed {
                logging::log_info(
                    &app_handle,
                    &format!("Backend installed (version: {:?})", version),
                );
            } else {
                logging::log_info(
                    &app_handle,
                    "Backend not installed. Waiting for frontend trigger...",
                );
                let _ = app_handle.emit("comfyui-download-start", ());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| if let tauri::RunEvent::Exit = event {
        let shutdown = _app_handle.state::<process::ShutdownSignal>();
        shutdown.shutdown.store(true, Ordering::Release);

        // Signal cancellation for any in-progress download
        if let Ok(guard) = _app_handle.state::<BackendState>().cancel_token.lock() {
            guard.store(true, Ordering::Release);
        }

        let log_sender = _app_handle.state::<logging::LogSender>();
        if let Ok(mut tx_guard) = log_sender.tx.lock() {
            *tx_guard = None;
        }

        let state = _app_handle.state::<process::ComfyUiState>();
        let mut lock = match state.child.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(mut child) = lock.take() {
            let pid_u32 = child.id();
            let pid = match i32::try_from(pid_u32) {
                Ok(p) if p > 0 => Some(p),
                _ => {
                    eprintln!("Invalid PID {} for process group kill, falling back to single-process kill", pid_u32);
                    None
                }
            };

            if let Some(pid) = pid {
                println!("Terminating ComfyUI process group: {}", pid);
                unsafe { libc::kill(-pid, libc::SIGTERM); }
                for _ in 0..10 {
                    if let Ok(Some(_)) = child.try_wait() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                unsafe { libc::kill(-pid, libc::SIGKILL); }

                // Wait with timeout instead of blocking indefinitely
                let wait_start = std::time::Instant::now();
                let wait_timeout = Duration::from_secs(3);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            println!("Process group {} exited with status: {:?}", pid, status);
                            break;
                        }
                        Ok(None) => {
                            if wait_start.elapsed() >= wait_timeout {
                                eprintln!(
                                    "Warning: Process group {} did not exit {}s after SIGKILL (likely in D state). \
                                     Abandoning wait to prevent shutdown freeze.",
                                    pid,
                                    wait_timeout.as_secs()
                                );
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        Err(e) => {
                            eprintln!("Error waiting for process group {}: {:?}", pid, e);
                            break;
                        }
                    }
                }
            } else {
                println!("Falling back to single-process kill for PID: {}", pid_u32);
                let _ = child.kill();

                // Same timeout for single-process fallback
                let wait_start = std::time::Instant::now();
                let wait_timeout = Duration::from_secs(3);
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) => {
                            if wait_start.elapsed() >= wait_timeout {
                                eprintln!("Warning: Single process {} did not exit, abandoning wait", pid_u32);
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let thread_handles = _app_handle.state::<process::ThreadHandles>();
        let mut handles_guard = match thread_handles.handles.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let handles: Vec<_> = handles_guard.drain(..).collect();
        drop(handles_guard);

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for handle in handles {
                let _ = handle.join();
            }
            let _ = tx.send(());
        });

        if rx.recv_timeout(Duration::from_secs(5)).is_err() {
            eprintln!("Warning: Shutdown timed out waiting for background threads to join.");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader;
    use std::path::{Path, PathBuf};

    // --- constants ---

    #[test]
    fn test_constants_range() {
        assert!((100..=100_000).contains(&logging::MAX_LOG_ENTRIES));
        assert!((10..=600).contains(&process::PORT_POLL_TIMEOUT_SECS));
        assert!((100..=10_000).contains(&process::PORT_POLL_INTERVAL_MS));
        assert!((100..=100_000).contains(&logging::LOG_CHANNEL_CAPACITY));
        assert!((10..=10_000).contains(&logging::BATCH_FLUSH_INTERVAL_MS));
        assert!((10..=1000).contains(&logging::BATCH_MAX_SIZE));
    }

    // --- downloader integration ---

    #[test]
    fn test_downloader_normalize_path_integration() {
        let result = downloader::normalize_path(Path::new("/x/y/../z/./w"));
        assert_eq!(result, PathBuf::from("/x/z/w"));
    }

    // --- BackendState ---

    #[test]
    fn test_backend_state_defaults() {
        let cancel = Arc::new(AtomicBool::new(false));
        let state = BackendState {
            install_dir: PathBuf::from("/tmp/test"),
            is_ready: AtomicBool::new(false),
            is_downloading: AtomicBool::new(false),
            cancel_token: Mutex::new(cancel),
        };
        assert!(!state.is_ready.load(Ordering::Relaxed));
        assert!(!state.is_downloading.load(Ordering::Relaxed));
    }

    // --- ShutdownSignal ---

    #[test]
    fn test_shutdown_signal_defaults() {
        let signal = process::ShutdownSignal {
            shutdown: AtomicBool::new(false),
        };
        assert!(!signal.shutdown.load(Ordering::Relaxed));
    }
}
