mod downloader;

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Manager, Emitter};

#[derive(serde::Deserialize, Default, Clone, Debug)]
struct AppConfig {
    python_path: Option<String>,
    comfyui_dir: Option<String>,
    extra_model_paths: Option<String>,
}

fn ensure_user_config(app_handle: &tauri::AppHandle) {
    if let Ok(config_dir) = app_handle.path().app_config_dir() {
        std::fs::create_dir_all(&config_dir).ok();
        let user_config = config_dir.join("config.yaml");
        if !user_config.exists() {
            let bundle = std::path::Path::new("../config.yaml.default");
            let resource = app_handle.path().resource_dir().ok().map(|d| d.join("config.yaml.default"));
            let src = if bundle.exists() { bundle.to_path_buf() }
                     else { resource.unwrap_or_default() };
            if src.exists() {
                std::fs::copy(&src, &user_config).ok();
            }
        }
    }
}

fn read_app_config(app_handle: &tauri::AppHandle) -> AppConfig {
    let check_path = |path: &std::path::Path| -> Option<AppConfig> {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = serde_yaml::from_str::<AppConfig>(&content) {
                    return Some(config);
                }
            }
        }
        None
    };

    if let Some(config) = check_path(&std::path::Path::new("config.yaml")) {
        return config;
    }

    if let Some(config) = check_path(&std::path::Path::new("../config.yaml")) {
        return config;
    }

    if let Ok(config_dir) = app_handle.path().app_config_dir() {
        let config_file = config_dir.join("config.yaml");
        if let Some(config) = check_path(&config_file) {
            return config;
        }
    }

    AppConfig::default()
}

const MAX_LOG_ENTRIES: usize = 2000;
const PORT_POLL_TIMEOUT_SECS: u32 = 60;
const PORT_POLL_INTERVAL_MS: u64 = 1000;
const PORT_CONNECT_TIMEOUT_MS: u64 = 200;
const LOG_CHANNEL_CAPACITY: usize = 1000;
const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
const BATCH_MAX_SIZE: usize = 50;

enum LogMessage {
    Stdout(String),
    Stderr(String),
    Launcher(String),
}

struct ComfyUiState {
    child: Mutex<Option<Child>>,
}

struct LogBuffer {
    logs: Mutex<VecDeque<(u64, String)>>,
    next_id: AtomicU64,
}

struct RedirectionState {
    is_redirected: AtomicBool,
}

struct LogSender {
    tx: Mutex<Option<SyncSender<LogMessage>>>,
}

struct ThreadHandles {
    handles: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

struct ShutdownSignal {
    shutdown: AtomicBool,
}

struct LogStats {
    dropped: AtomicU64,
    total_received: AtomicU64,
}

struct BackendState {
    install_dir: std::path::PathBuf,
    is_ready: AtomicBool,
    is_downloading: AtomicBool,
    cancel_token: Mutex<Arc<AtomicBool>>,
}

fn log_info(app_handle: &tauri::AppHandle, message: &str) {
    if let Some(sender) = app_handle.try_state::<LogSender>() {
        if let Ok(tx_guard) = sender.tx.lock() {
            if let Some(ref tx) = *tx_guard {
                if tx
                    .try_send(LogMessage::Launcher(message.to_string()))
                    .is_err()
                {
                    eprintln!("[Launcher] {}", message);
                }
            }
        }
    } else {
        println!("[Launcher] {}", message);
    }
}

fn flush_batch(app_handle: &tauri::AppHandle, batch: &mut Vec<(&'static str, String)>) {
    if batch.is_empty() {
        return;
    }
    let mut grouped: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    for (event, msg) in batch.drain(..) {
        grouped.entry(event).or_default().push(msg);
    }
    for (event, messages) in grouped {
        let _ = app_handle.emit(event, messages);
    }
}

#[tauri::command]
fn get_logs(
    log_buffer: tauri::State<'_, LogBuffer>,
    last_id: Option<u64>,
) -> (Vec<(u64, String)>, u64) {
    let logs = match log_buffer.logs.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let last_id = last_id.unwrap_or(0);
    let new_logs: Vec<_> = logs
        .iter()
        .filter(|(id, _)| *id > last_id)
        .cloned()
        .collect();

    let max_id = new_logs.iter().map(|(id, _)| *id).max().unwrap_or(last_id);

    (new_logs, max_id)
}

#[tauri::command]
fn get_log_stats(stats: tauri::State<'_, LogStats>) -> (u64, u64) {
    (
        stats.total_received.load(Ordering::Relaxed),
        stats.dropped.load(Ordering::Relaxed),
    )
}

#[tauri::command]
fn check_backend_status(
  app_handle: tauri::AppHandle,
  backend: tauri::State<'_, BackendState>,
) -> (bool, Option<String>) {
  let user_config = read_app_config(&app_handle);
  
  if let (Some(ref py_path), Some(ref comfy_path)) = (&user_config.python_path, &user_config.comfyui_dir) {
    if std::path::Path::new(py_path).exists() && std::path::Path::new(comfy_path).exists() {
      return (true, Some("custom".to_string()));
    }
  }

  let installer = downloader::BackendInstaller::new(
    backend.install_dir.clone(),
    downloader::backend_download_url(),
    None,
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

fn detect_dgpu_index() -> String {
    let output = std::process::Command::new("rocm-smi")
        .arg("--showmeminfo")
        .arg("vram")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut max_vram = 0u64;
            let mut best_gpu = "0".to_string();

            for line in stdout.lines() {
                if line.contains("VRAM Total Memory") {
                    if let (Some(start_idx), Some(end_idx)) = (line.find('['), line.find(']')) {
                        let gpu_num = &line[start_idx + 1..end_idx];
                        if let Some(col_idx) = line.rfind(':') {
                            let vram_str = line[col_idx + 1..].trim();
                            if let Ok(vram_bytes) = vram_str.parse::<u64>() {
                                eprintln!(
                                    "[GPU Detection] GPU {}: {} bytes VRAM",
                                    gpu_num, vram_bytes
                                );
                                if vram_bytes > max_vram {
                                    max_vram = vram_bytes;
                                    best_gpu = gpu_num.to_string();
                                }
                            } else {
                                eprintln!(
                                    "[GPU Detection] Failed to parse VRAM value: '{}'",
                                    vram_str
                                );
                            }
                        }
                    }
                }
            }
            best_gpu
        }
        Ok(out) => {
            eprintln!(
                "rocm-smi exited with error: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            "0".to_string()
        }
        Err(e) => {
            eprintln!("rocm-smi not found or failed to execute: {:?}", e);
            "0".to_string()
        }
    }
}

/// Detect if the GPU needs an HSA_OVERRIDE_GFX_VERSION environment variable.
/// Uses sysfs topology nodes first, falls back to rocminfo.
fn detect_hsa_override() -> Option<&'static str> {
    let topology_base = Path::new("/sys/class/kfd/kfd/topology/nodes");

    if topology_base.exists() {
        if let Ok(entries) = std::fs::read_dir(topology_base) {
            for entry in entries.flatten() {
                let gfx_path = entry.path().join("gfx_target_version");
                if let Ok(content) = std::fs::read_to_string(&gfx_path) {
                    let ver_str = content.trim();
                    if ver_str.is_empty() || ver_str == "0" {
                        continue;
                    }
                    if let Ok(version) = ver_str.parse::<u32>() {
                        let major = version / 10000;
                        let _minor = (version / 100) % 100;
                        let patch = version % 100;

                        eprintln!(
                            "[GPU Detection] sysfs node {}: gfx_target_version={} → gfx{}.{}.{}",
                            entry.file_name().to_string_lossy(),
                            version,
                            major,
                            _minor,
                            patch
                        );

                        if patch > 0 {
                            return match major {
                                10 => Some("10.3.0"),
                                11 => Some("11.0.0"),
                                _ => None,
                            };
                        }
                        return None;
                    }
                }
            }
        }
        return None;
    }

    detect_hsa_override_fallback()
}

fn detect_hsa_override_fallback() -> Option<&'static str> {
    let output = std::process::Command::new("rocminfo").output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if let Some(pos) = line.rfind("gfx") {
                    let version: String = line[pos + 3..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if !version.is_empty() {
                        return parse_hsa_override(&version);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_hsa_override(gfx: &str) -> Option<&'static str> {
    if gfx.len() == 4 && gfx.chars().all(|c| c.is_ascii_digit()) {
        let major = &gfx[0..2];
        let patch = &gfx[3..4];
        if patch != "0" {
            return match major {
                "10" => Some("10.3.0"),
                "11" => Some("11.0.0"),
                "12" => Some("12.0.0"), // RDNA4 future-proof
                _ => None,              // Unknown major: don't guess
            };
        }
        return None;
    }

    let parts: Vec<&str> = gfx.split('.').collect();
    if parts.len() == 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            if patch > 0 {
                let major = parts[0].parse::<u32>().unwrap_or(10);
                return match major {
                    10 => Some("10.3.0"),
                    11 => Some("11.0.0"),
                    12 => Some("12.0.0"), // RDNA4
                    _ => None,            // Unknown major: safe default
                };
            }
        }
    }

    None
}

fn spawn_comfyui_process(app_handle: &tauri::AppHandle) -> Result<(), String> {
  let user_config = read_app_config(app_handle);
  let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

  let python_path = if let Some(ref path) = user_config.python_path {
    std::path::PathBuf::from(path)
  } else {
    let mut p = current_dir.join("venv").join("bin").join("python");
    if !p.exists() {
      if let Ok(res_dir) = app_handle.path().resource_dir() {
        let test_path = res_dir.join("venv").join("bin").join("python");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    if !p.exists() {
      if let Some(install_dir) = downloader::default_install_dir() {
        let test_path = install_dir.join("venv/bin/python");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    p
  };

  let comfyui_dir = if let Some(ref path) = user_config.comfyui_dir {
    std::path::PathBuf::from(path)
  } else {
    let mut p = current_dir.join("ComfyUI");
    if !p.exists() {
      if let Ok(res_dir) = app_handle.path().resource_dir() {
        let test_path = res_dir.join("ComfyUI");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    if !p.exists() {
      if let Some(install_dir) = downloader::default_install_dir() {
        let test_path = install_dir.join("ComfyUI");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    p
  };

  let extra_model_paths = if let Some(ref path) = user_config.extra_model_paths {
    std::path::PathBuf::from(path)
  } else {
    let mut p = current_dir.join("extra_model_paths.yaml");
    if !p.exists() {
      if let Ok(res_dir) = app_handle.path().resource_dir() {
        let test_path = res_dir.join("extra_model_paths.yaml");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    if !p.exists() {
      if let Some(install_dir) = downloader::default_install_dir() {
        let test_path = install_dir.join("extra_model_paths.yaml");
        if test_path.exists() {
          p = test_path;
        }
      }
    }
    p
  };

    log_info(app_handle, &format!("Python Path: {:?}", python_path));
    log_info(app_handle, &format!("Working Directory: {:?}", comfyui_dir));

    let gpu_index = detect_dgpu_index();
    let hsa_override = detect_hsa_override();
    log_info(
        app_handle,
        &format!(
            "Smart GPU Detection: Using GPU {} (discrete GPU with largest VRAM)",
            gpu_index
        ),
    );
    if let Some(ver) = hsa_override {
        log_info(
            app_handle,
            &format!(
                "HSA_OVERRIDE_GFX_VERSION={} (GPU variant requires override)",
                ver
            ),
        );
    }

    let mut cmd = std::process::Command::new(&python_path);
    cmd.arg("main.py")
        .arg("--extra-model-paths-config")
        .arg(&extra_model_paths)
        .current_dir(&comfyui_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("HIP_VISIBLE_DEVICES", &gpu_index)
        .process_group(0);

    if let Some(hsa_ver) = hsa_override {
        cmd.env("HSA_OVERRIDE_GFX_VERSION", hsa_ver);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => {
            log_info(
                app_handle,
                &format!("Python process spawned with PID: {}", child.id()),
            );
            child
        }
        Err(err) => {
            return Err(format!("Failed to start ComfyUI Python process: {:?}", err));
        }
    };

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let stderr = child.stderr.take().expect("Failed to open stderr");

    let app_handle_stdout = app_handle.clone();
    let app_handle_stderr = app_handle.clone();

    // Acquire LogSender ONCE, then clone for both stdout/stderr threads
    let log_tx = match app_handle.state::<LogSender>().tx.lock() {
        Ok(guard) => match guard.clone() {
            Some(tx) => tx,
            None => return Err("LogSender already dropped".to_string()),
        },
        Err(poisoned) => match poisoned.into_inner().clone() {
            Some(tx) => tx,
            None => return Err("LogSender poisoned and empty".to_string()),
        },
    };
    let log_tx_stdout = log_tx.clone();
    let log_tx_stderr = log_tx;

    let stdout_handle = std::thread::spawn(move || {
        let stats = app_handle_stdout.state::<LogStats>();
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            stats.total_received.fetch_add(1, Ordering::Relaxed);
            if let Ok(line_str) = line {
                if log_tx_stdout
                    .try_send(LogMessage::Stdout(line_str))
                    .is_err()
                {
                    stats.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        let stats = app_handle_stderr.state::<LogStats>();
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            stats.total_received.fetch_add(1, Ordering::Relaxed);
            if let Ok(line_str) = line {
                if log_tx_stderr
                    .try_send(LogMessage::Stderr(line_str))
                    .is_err()
                {
                    stats.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });

    if let Ok(mut handles) = app_handle.state::<ThreadHandles>().handles.lock() {
        handles.push(stdout_handle);
        handles.push(stderr_handle);
    }

    {
        let state = app_handle.state::<ComfyUiState>();
        let mut lock = match state.child.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *lock = Some(child);
    }

    // Spawn port polling thread
    let app_handle_poll = app_handle.clone();
    let polling_handle = std::thread::spawn(move || {
        let port_addr = "127.0.0.1:8188";
        let state = app_handle_poll.state::<ComfyUiState>();
        let shutdown = app_handle_poll.state::<ShutdownSignal>();

        log_info(
            &app_handle_poll,
            &format!("Starting port check on {}...", port_addr),
        );

        let socket_addr: std::net::SocketAddr = match port_addr.parse() {
            Ok(addr) => addr,
            Err(_) => {
                log_info(&app_handle_poll, "Failed to parse port address.");
                return;
            }
        };

        let mut count = 0u32;
        let mut timeout_emitted = false;
        loop {
            if shutdown.shutdown.load(Ordering::Acquire) {
                break;
            }

            {
                let mut lock = match state.child.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                if let Some(ref mut child) = *lock {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let err_msg = format!(
                                "Python process exited unexpectedly with status: {:?}",
                                status
                            );
                            log_info(&app_handle_poll, &err_msg);
                            let _ = app_handle_poll.emit("comfyui-error", err_msg);
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log_info(
                                &app_handle_poll,
                                &format!("Failed to check child process status: {:?}", e),
                            );
                        }
                    }
                }
            }

            if std::net::TcpStream::connect_timeout(
                &socket_addr,
                std::time::Duration::from_millis(PORT_CONNECT_TIMEOUT_MS),
            )
            .is_ok()
            {
                log_info(
                    &app_handle_poll,
                    &format!(
                        "ComfyUI server responsive on {}. Redirecting window...",
                        port_addr
                    ),
                );

                if let Some(window) = app_handle_poll.get_webview_window("main") {
                    if let Ok(url) = "http://127.0.0.1:8188".parse::<tauri::Url>() {
                        let redirect_state = app_handle_poll.state::<RedirectionState>();
                        redirect_state.is_redirected.store(true, Ordering::Release);
                        if let Err(e) = window.navigate(url) {
                            log_info(
                                &app_handle_poll,
                                &format!("Error navigating webview window: {:?}", e),
                            );
                        }
                    } else {
                        log_info(&app_handle_poll, "Failed to parse target URL.");
                    }
                } else {
                    log_info(&app_handle_poll, "Webview window 'main' not found.");
                }
                break;
            }

            count += 1;
            if count >= PORT_POLL_TIMEOUT_SECS && !timeout_emitted {
                let timeout_msg = format!(
                    "Failed to connect to ComfyUI after {} seconds. Please check the logs below.",
                    PORT_POLL_TIMEOUT_SECS
                );
                log_info(&app_handle_poll, &timeout_msg);
                let _ = app_handle_poll.emit("comfyui-timeout", timeout_msg);
                timeout_emitted = true;
            } else if count > 0 && count % 300 == 0 {
                log_info(
                    &app_handle_poll,
                    &format!("Still waiting... {} seconds elapsed.", count),
                );
            }

            std::thread::sleep(std::time::Duration::from_millis(PORT_POLL_INTERVAL_MS));
        }
    });

    if let Ok(mut handles) = app_handle.state::<ThreadHandles>().handles.lock() {
        handles.push(polling_handle);
    }

    Ok(())
}

#[tauri::command]
fn start_comfyui(app_handle: tauri::AppHandle) -> Result<(), String> {
    {
        let state = app_handle.state::<ComfyUiState>();
        let lock = match state.child.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(ref existing_child) = *lock {
            return Err(format!(
                "ComfyUI is already running (PID {})",
                existing_child.id()
            ));
        }
    }
    spawn_comfyui_process(&app_handle)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    std::env::set_var("WEBKIT_FORCE_COMPOSITING_MODE", "1");
    std::env::set_var("GIO_USE_PROXY_RESOLVER", "dummy");
    std::env::set_var("no_proxy", "*");
    std::env::set_var("NO_PROXY", "*");

    let (log_tx, log_rx) = std::sync::mpsc::sync_channel::<LogMessage>(LOG_CHANNEL_CAPACITY);

    let app = tauri::Builder::default()
        .manage(ComfyUiState {
            child: Mutex::new(None),
        })
        .manage(LogBuffer {
            logs: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(0),
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
            dropped: AtomicU64::new(0),
            total_received: AtomicU64::new(0),
        })
        .manage(BackendState {
            install_dir: downloader::default_install_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
            is_ready: AtomicBool::new(false),
            is_downloading: AtomicBool::new(false),
            cancel_token: Mutex::new(Arc::new(AtomicBool::new(false))),
        })
        .invoke_handler(tauri::generate_handler![
            get_logs,
            get_log_stats,
            check_backend_status,
            start_backend_download,
            cancel_backend_download,
            start_comfyui,
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

            ensure_user_config(&app_handle);

            // Writer thread: receives MPSC messages, formats, stores, and emits batched to frontend
            let app_handle_writer = app_handle.clone();
            let writer_handle = std::thread::spawn(move || {
                let log_buffer = app_handle_writer.state::<LogBuffer>();
                let redirect_state = app_handle_writer.state::<RedirectionState>();

                let mut batch: Vec<(&'static str, String)> = Vec::with_capacity(BATCH_MAX_SIZE);

                loop {
                    match log_rx.recv_timeout(Duration::from_millis(BATCH_FLUSH_INTERVAL_MS)) {
                        Ok(msg) => {
                            let is_redirected =
                                redirect_state.is_redirected.load(Ordering::Acquire);

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

                            let id = log_buffer.next_id.fetch_add(1, Ordering::Relaxed);
                            let mut logs = match log_buffer.logs.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            if logs.len() >= MAX_LOG_ENTRIES {
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

                            if batch.len() >= BATCH_MAX_SIZE {
                                flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                flush_batch(&app_handle_writer, &mut batch);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }
                }

                flush_batch(&app_handle_writer, &mut batch);
            });

            if let Ok(mut handles) = app_handle.state::<ThreadHandles>().handles.lock() {
                handles.push(writer_handle);
            }

            log_info(&app_handle, "Starting ComfyUI Desktop Launcher...");

            let user_config = read_app_config(&app_handle);
            let is_custom = if let (Some(ref py_path), Some(ref comfy_path)) = (&user_config.python_path, &user_config.comfyui_dir) {
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
                    None,
                );
                (installer.is_installed(), installer.installed_version())
            };

            if is_installed {
                log_info(
                    &app_handle,
                    &format!("Backend installed (version: {:?})", version),
                );
                // Frontend handles this via check_backend_status() in checkAndStart()
            } else {
                log_info(
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
      let shutdown = _app_handle.state::<ShutdownSignal>();
      shutdown.shutdown.store(true, Ordering::Release);

      // Signal cancellation for any in-progress download
      if let Ok(guard) = _app_handle.state::<BackendState>().cancel_token.lock() {
        guard.store(true, Ordering::Release);
      }

      let log_sender = _app_handle.state::<LogSender>();
      if let Ok(mut tx_guard) = log_sender.tx.lock() {
        *tx_guard = None;
      }

      let state = _app_handle.state::<ComfyUiState>();
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
        } else {
          println!("Falling back to single-process kill for PID: {}", pid_u32);
          let _ = child.kill();
        }
        let _ = child.wait();
      }

      let thread_handles = _app_handle.state::<ThreadHandles>();
      let mut handles = match thread_handles.handles.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
      };
      for handle in handles.drain(..) {
        let _ = handle.join();
      }
    }
  );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader;
    use std::path::{Path, PathBuf};

    // --- parse_hsa_override ---

    #[test]
    fn test_parse_rdna2_native() {
        assert_eq!(parse_hsa_override("1030"), None);
    }

    #[test]
    fn test_parse_rdna2_patched() {
        assert_eq!(parse_hsa_override("1031"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_rdna2_other_patches() {
        assert_eq!(parse_hsa_override("1032"), Some("10.3.0"));
        assert_eq!(parse_hsa_override("1033"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_rdna3_1100_native() {
        assert_eq!(parse_hsa_override("1100"), None);
    }

    #[test]
    fn test_parse_rdna3_1101_patched() {
        assert_eq!(parse_hsa_override("1101"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_rdna3_1102() {
        assert_eq!(parse_hsa_override("1102"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_rdna3_1150_native() {
        assert_eq!(parse_hsa_override("1150"), None);
    }

    #[test]
    fn test_parse_rdna3_1151_patched() {
        assert_eq!(parse_hsa_override("1151"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_dotted_rdna2() {
        assert_eq!(parse_hsa_override("10.3.1"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_dotted_rdna3() {
        assert_eq!(parse_hsa_override("11.0.1"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_dotted_unknown_major() {
        assert_eq!(parse_hsa_override("12.0.1"), Some("12.0.0"));
        assert_eq!(parse_hsa_override("9.0.1"), None);
    }

    #[test]
    fn test_parse_unknown_major_returns_none() {
        assert_eq!(parse_hsa_override("9931"), None);
        assert_eq!(parse_hsa_override("1301"), None);
    }

    #[test]
    fn test_parse_rdna4_future() {
        assert_eq!(parse_hsa_override("1200"), None);
        assert_eq!(parse_hsa_override("1201"), Some("12.0.0"));
        assert_eq!(parse_hsa_override("12.0.1"), Some("12.0.0"));
    }

    #[test]
    fn test_parse_dotted_no_patch() {
        assert_eq!(parse_hsa_override("10.3.0"), None);
        assert_eq!(parse_hsa_override("11.0.0"), None);
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_hsa_override("invalid"), None);
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_hsa_override(""), None);
    }

    #[test]
    fn test_parse_short() {
        assert_eq!(parse_hsa_override("103"), None);
    }

    // --- constants ---

    #[test]
    fn test_constants_range() {
        assert!((100..=100_000).contains(&MAX_LOG_ENTRIES));
        assert!((10..=600).contains(&PORT_POLL_TIMEOUT_SECS));
        assert!((100..=10_000).contains(&PORT_POLL_INTERVAL_MS));
        assert!((100..=100_000).contains(&LOG_CHANNEL_CAPACITY));
        assert!((10..=10_000).contains(&BATCH_FLUSH_INTERVAL_MS));
        assert!((10..=1000).contains(&BATCH_MAX_SIZE));
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
        let signal = ShutdownSignal {
            shutdown: AtomicBool::new(false),
        };
        assert!(!signal.shutdown.load(Ordering::Relaxed));
    }
}
