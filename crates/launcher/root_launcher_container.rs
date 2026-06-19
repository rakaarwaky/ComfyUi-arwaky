// PURPOSE: launcher — root DI container. Wires concrete implementations into ports/protocols,
// builds the orchestrator, and configures the Tauri app. This is the ONLY place that
// imports both contracts (from shared) and concrete implementations (from feature crates).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

use launcher_shared::contract_backend_install_protocol::BackendInstallProtocol;
use launcher_shared::contract_config_port::ConfigPort;
use launcher_shared::contract_gpu_detection_protocol::GpuDetectionProtocol;
use launcher_shared::contract_gpu_monitor_port::GpuMonitorPort;
use launcher_shared::contract_launcher_aggregate::LauncherAggregate;
use launcher_shared::contract_log_emitter_port::LogEmitterPort;
use launcher_shared::contract_log_writer_port::LogWriterPort;
use launcher_shared::{BackendStatus, LogSource};

use launcher_backend_engine::BackendInstaller;
use launcher_config::ConfigLoader;
use launcher_engine::LauncherOrchestrator;
use launcher_gpu_detector::GpuDetector;
use launcher_log_engine::LogEmitter;
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

/// Flush a batch of log messages to the Tauri frontend, grouped by event name.
fn flush_batch(app_handle: &tauri::AppHandle, batch: &mut Vec<(&'static str, String)>) {
    if batch.is_empty() {
        return;
    }
    let mut grouped: HashMap<&'static str, Vec<String>> = HashMap::new();
    for (event, msg) in batch.drain(..) {
        grouped.entry(event).or_default().push(msg);
    }
    for (event, messages) in grouped {
        let _ = app_handle.emit(event, messages);
    }
}

/// Configure the Tauri app with all managed state, invoke handlers, and setup.
pub fn configure_app(
    log_tx: std::sync::mpsc::SyncSender<LogMessage>,
    log_rx: std::sync::mpsc::Receiver<LogMessage>,
) -> tauri::Builder<tauri::Wry> {
    let (gpu_adapter, _gpu_metrics) = launcher_log_engine::GpuMonitorAdapter::new();
    gpu_adapter.start_polling();
    let gpu_port: Box<dyn GpuMonitorPort> = Box::new(gpu_adapter);

    let log_emitter = Arc::new(LogEmitter::new(log_tx.clone()));

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
        .manage(gpu_port)
        .manage(log_emitter)
        .manage(launcher_log_engine::StartTime(std::time::Instant::now()))
        .invoke_handler(tauri::generate_handler![
            launcher_log_engine::get_logs,
            launcher_log_engine::get_log_stats,
            launcher_log_engine::get_health,
            launcher_log_engine::get_gpu_metrics,
            launcher_log_engine::open_log_viewer,
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
            let file_writer = launcher_log_engine::FileLogWriter::new(&config_dir);
            let file_writer_ref = file_writer.clone();
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
                            let formatted = msg.to_string();
                            let is_stderr = msg.source == LogSource::Stderr;

                            // Always write to file (even after redirect)
                            file_writer_ref.write_log(&formatted);

                            let is_redirected = redirect_state
                                .is_redirected
                                .load(std::sync::atomic::Ordering::Acquire);
                            if is_redirected {
                                continue;
                            }

                            #[cfg(debug_assertions)]
                            if is_stderr {
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

                            let event_name: &'static str = match msg.source {
                                LogSource::Stdout => "comfyui-log-stdout",
                                LogSource::Stderr => "comfyui-log-stderr",
                                LogSource::Launcher => "comfyui-log",
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

            // Log startup via LogEmitter port
            let emitter = app_handle.state::<Arc<dyn LogEmitterPort>>();
            emitter.log_info("Starting ComfyUI Desktop Launcher...");

            // Check backend status on startup
            let agg = app.state::<Arc<dyn LauncherAggregate>>();
            match agg.check_backend_status() {
                BackendStatus::Installed { version } => {
                    emitter.log_info(&format!("Backend installed (version: {:?})", version));
                }
                BackendStatus::CustomInstall => {
                    emitter.log_info("Backend: custom install detected");
                }
                BackendStatus::NotInstalled => {
                    emitter.log_info("Backend not installed. Waiting for frontend trigger...");
                    let _ = app_handle.emit("comfyui-download-start", ());
                }
            }

            Ok(())
        })
}
