// PURPOSE: launcher — root DI container. Wires concrete implementations into ports/protocols,
// builds the orchestrator, and configures the Tauri app. This is the ONLY place that
// imports both contracts (from shared) and concrete implementations (from feature crates).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

use launcher_shared::contract_backend_install_protocol::BackendInstallProtocol;
use launcher_shared::contract_config_port::ConfigPort;
use launcher_shared::contract_gpu_detection_protocol::GpuDetectionProtocol;
use launcher_shared::contract_launcher_aggregate::LauncherAggregate;
use launcher_shared::BackendStatus;

use launcher_backend_engine::BackendInstaller;
use launcher_config::ConfigLoader;
use launcher_engine::LauncherOrchestrator;
use launcher_gpu_detector::GpuDetector;
use launcher_log_engine::flush_batch;
use launcher_process_manager::ProcessSpawner;
use launcher_shared::{
    ComfyUiState, InstallDir, LogBuffer, LogMessage, LogSender, LogStats, RedirectionState,
    ShutdownSignal, ThreadHandles,
};

pub struct DownloadState {
    pub cancel_token: Mutex<Option<Arc<std::sync::atomic::AtomicBool>>>,
    pub is_downloading: AtomicBool,
}

/// Build the aggregate (orchestrator) with all concrete dependencies wired.
fn build_aggregate(config_dir: std::path::PathBuf) -> Arc<dyn LauncherAggregate> {
    let config_path = config_dir.join("config.yaml");
    let config_port: Arc<dyn ConfigPort> = Arc::new(ConfigLoader::new(config_path));
    let backend_protocol: Arc<dyn BackendInstallProtocol> = Arc::new(BackendInstaller);
    let gpu_protocol: Arc<dyn GpuDetectionProtocol> = Arc::new(GpuDetector);
    let process_port: Arc<dyn launcher_shared::contract_process_port::ProcessPort> =
        Arc::new(ProcessSpawner);

    Arc::new(LauncherOrchestrator::new(
        config_port,
        backend_protocol,
        gpu_protocol,
        process_port,
    ))
}

/// Configure the Tauri app with all managed state, invoke handlers, and setup.
pub fn configure_app(
    log_tx: std::sync::mpsc::SyncSender<LogMessage>,
    log_rx: std::sync::mpsc::Receiver<LogMessage>,
) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .manage(ComfyUiState {
            child: Mutex::new(None),
        })
        .manage(LogBuffer {
            logs: Mutex::new(std::collections::VecDeque::new()),
            next_id: std::sync::atomic::AtomicU64::new(0),
        })
        .manage(RedirectionState {
            is_redirected: AtomicBool::new(false),
        })
        .manage(LogSender {
            tx: Mutex::new(Some(log_tx)),
        })
        .manage(ThreadHandles {
            handles: Mutex::new(Vec::new()),
        })
        .manage(ShutdownSignal {
            shutdown: AtomicBool::new(false),
        })
        .manage(LogStats {
            dropped: std::sync::atomic::AtomicU64::new(0),
            total_received: std::sync::atomic::AtomicU64::new(0),
        })
        .manage(DownloadState {
            cancel_token: Mutex::new(None),
            is_downloading: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            launcher_log_engine::get_logs,
            launcher_log_engine::get_log_stats,
            crate::check_backend_status,
            crate::start_backend_download,
            crate::cancel_backend_download,
            crate::start_comfyui,
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

            // Build and store aggregate — ensure will init config
            let config_dir = app_handle.path().app_config_dir().unwrap_or_default();
            let agg = build_aggregate(config_dir.clone());
            agg.ensure_config(&InstallDir(config_dir.clone()));
            app.manage::<Arc<dyn LauncherAggregate>>(agg);

            let app_handle_writer = app_handle.clone();
            let writer_handle = std::thread::spawn(move || {
                let log_buffer = app_handle_writer.state::<LogBuffer>();
                let redirect_state = app_handle_writer.state::<RedirectionState>();
                let mut batch: Vec<(&'static str, String)> =
                    Vec::with_capacity(launcher_log_engine::BATCH_MAX_SIZE);

                loop {
                    match log_rx.recv_timeout(Duration::from_millis(
                        launcher_log_engine::BATCH_FLUSH_INTERVAL_MS,
                    )) {
                        Ok(msg) => {
                            let is_redirected = redirect_state
                                .is_redirected
                                .load(std::sync::atomic::Ordering::Acquire);
                            if is_redirected {
                                continue;
                            }

                            let (formatted, _is_stderr) = match msg {
                                LogMessage::Stdout(ref line) => {
                                    (format!("[stdout] {}", line), false)
                                }
                                LogMessage::Stderr(ref line) => {
                                    (format!("[stderr] {}", line), true)
                                }
                                LogMessage::Launcher(ref line) => {
                                    (format!("[Launcher] {}", line), false)
                                }
                            };

                            #[cfg(debug_assertions)]
                            if _is_stderr {
                                eprintln!("{}", formatted);
                            } else {
                                println!("{}", formatted);
                            }

                            let id = log_buffer
                                .next_id
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let mut logs = match log_buffer.logs.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            if logs.len() >= launcher_log_engine::MAX_LOG_ENTRIES {
                                logs.pop_front();
                            }
                            logs.push_back((id, formatted.clone()));
                            drop(logs);

                            let event_name: &'static str = match msg {
                                LogMessage::Stdout(_) => "comfyui-log-stdout",
                                LogMessage::Stderr(_) => "comfyui-log-stderr",
                                LogMessage::Launcher(_) => "comfyui-log",
                            };
                            batch.push((event_name, formatted));

                            if batch.len() >= launcher_log_engine::BATCH_MAX_SIZE {
                                flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                flush_batch(&app_handle_writer, &mut batch);
            });

            if let Ok(mut handles) = app_handle.state::<ThreadHandles>().handles.lock() {
                handles.push(writer_handle);
            }

            launcher_log_engine::log_info(&app_handle, "Starting ComfyUI Desktop Launcher...");

            // Check backend status on startup
            let agg = app.state::<Arc<dyn LauncherAggregate>>();
            match agg.check_backend_status() {
                BackendStatus::Installed { version } => {
                    launcher_log_engine::log_info(
                        &app_handle,
                        &format!("Backend installed (version: {:?})", version),
                    );
                }
                BackendStatus::CustomInstall => {
                    launcher_log_engine::log_info(&app_handle, "Backend: custom install detected");
                }
                BackendStatus::NotInstalled => {
                    launcher_log_engine::log_info(
                        &app_handle,
                        "Backend not installed. Waiting for frontend trigger...",
                    );
                    let _ = app_handle.emit("comfyui-download-start", ());
                }
            }

            Ok(())
        })
}
