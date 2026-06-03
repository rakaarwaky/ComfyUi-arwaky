use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::thread::JoinHandle;
use tauri::{Emitter, Manager};

pub const PORT_POLL_TIMEOUT_SECS: u32 = 60;
pub const PORT_POLL_INTERVAL_MS: u64 = 1000;
pub const PORT_CONNECT_TIMEOUT_MS: u64 = 200;

pub struct ComfyUiState {
    pub child: Mutex<Option<Child>>,
}

pub struct RedirectionState {
    pub is_redirected: AtomicBool,
}

pub struct ThreadHandles {
    pub handles: Mutex<Vec<JoinHandle<()>>>,
}

pub struct ShutdownSignal {
    pub shutdown: AtomicBool,
}

pub fn spawn_comfyui_process(app_handle: &tauri::AppHandle) -> Result<std::process::Child, String> {
    let user_config = crate::config::read_app_config(app_handle);
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
            if let Some(install_dir) = crate::downloader::default_install_dir() {
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
            if let Some(install_dir) = crate::downloader::default_install_dir() {
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
            if let Some(install_dir) = crate::downloader::default_install_dir() {
                let test_path = install_dir.join("extra_model_paths.yaml");
                if test_path.exists() {
                    p = test_path;
                }
            }
        }
        p
    };

    let output_dir = user_config
        .output_dir
        .as_ref()
        .map(std::path::PathBuf::from);
    let input_dir = user_config.input_dir.as_ref().map(std::path::PathBuf::from);
    let user_dir = user_config.user_dir.as_ref().map(std::path::PathBuf::from);

    crate::logging::log_info(app_handle, &format!("Python Path: {:?}", python_path));
    crate::logging::log_info(app_handle, &format!("Working Directory: {:?}", comfyui_dir));
    if let Some(ref d) = output_dir {
        crate::logging::log_info(app_handle, &format!("Output Directory: {:?}", d));
    }
    if let Some(ref d) = input_dir {
        crate::logging::log_info(app_handle, &format!("Input Directory: {:?}", d));
    }
    if let Some(ref d) = user_dir {
        crate::logging::log_info(app_handle, &format!("User Directory: {:?}", d));
    }

    let gpu_index = crate::gpu::detect_dgpu_index();
    let hsa_override = crate::gpu::detect_hsa_override();
    crate::logging::log_info(
        app_handle,
        &format!(
            "Smart GPU Detection: Using GPU {} (discrete GPU with largest VRAM)",
            gpu_index
        ),
    );
    if let Some(ver) = hsa_override {
        crate::logging::log_info(
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
        .arg(&extra_model_paths);
    if let Some(ref out_dir) = output_dir {
        cmd.arg("--output-directory").arg(out_dir);
    }
    if let Some(ref in_dir) = input_dir {
        cmd.arg("--input-directory").arg(in_dir);
    }
    if let Some(ref u_dir) = user_dir {
        cmd.arg("--user-directory").arg(u_dir);
    }
    cmd.current_dir(&comfyui_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("HIP_VISIBLE_DEVICES", &gpu_index)
        .process_group(0);

    if let Some(hsa_ver) = hsa_override {
        cmd.env("HSA_OVERRIDE_GFX_VERSION", hsa_ver);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => {
            crate::logging::log_info(
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
    let log_tx = match app_handle.state::<crate::logging::LogSender>().tx.lock() {
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
        let stats = app_handle_stdout.state::<crate::logging::LogStats>();
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            stats
                .total_received
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(line_str) = line {
                if log_tx_stdout
                    .try_send(crate::logging::LogMessage::Stdout(line_str))
                    .is_err()
                {
                    stats
                        .dropped
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        let stats = app_handle_stderr.state::<crate::logging::LogStats>();
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            stats
                .total_received
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(line_str) = line {
                if log_tx_stderr
                    .try_send(crate::logging::LogMessage::Stderr(line_str))
                    .is_err()
                {
                    stats
                        .dropped
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });

    if let Ok(mut handles) = app_handle.state::<ThreadHandles>().handles.lock() {
        handles.push(stdout_handle);
        handles.push(stderr_handle);
    }

    // Lock updating is moved to start_comfyui to prevent race conditions

    // Spawn port polling thread
    let app_handle_poll = app_handle.clone();
    let polling_handle = std::thread::spawn(move || {
        let port_addr = "127.0.0.1:8188";
        let state = app_handle_poll.state::<ComfyUiState>();
        let shutdown = app_handle_poll.state::<ShutdownSignal>();

        crate::logging::log_info(
            &app_handle_poll,
            &format!("Starting port check on {}...", port_addr),
        );

        let socket_addr: std::net::SocketAddr = match port_addr.parse() {
            Ok(addr) => addr,
            Err(_) => {
                crate::logging::log_info(&app_handle_poll, "Failed to parse port address.");
                return;
            }
        };

        let mut count = 0u32;
        let mut timeout_emitted = false;
        loop {
            if shutdown.shutdown.load(std::sync::atomic::Ordering::Acquire) {
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
                            crate::logging::log_info(&app_handle_poll, &err_msg);
                            let _ = app_handle_poll.emit("comfyui-error", err_msg);
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            crate::logging::log_info(
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
                crate::logging::log_info(
                    &app_handle_poll,
                    &format!(
                        "ComfyUI server responsive on {}. Redirecting window...",
                        port_addr
                    ),
                );

                if let Some(window) = app_handle_poll.get_webview_window("main") {
                    if let Ok(url) = "http://127.0.0.1:8188".parse::<tauri::Url>() {
                        let redirect_state = app_handle_poll.state::<RedirectionState>();
                        redirect_state
                            .is_redirected
                            .store(true, std::sync::atomic::Ordering::Release);
                        if let Err(e) = window.navigate(url) {
                            crate::logging::log_info(
                                &app_handle_poll,
                                &format!("Error navigating webview window: {:?}", e),
                            );
                        }
                    } else {
                        crate::logging::log_info(&app_handle_poll, "Failed to parse target URL.");
                    }
                } else {
                    crate::logging::log_info(&app_handle_poll, "Webview window 'main' not found.");
                }
                break;
            }

            count += 1;
            if count >= PORT_POLL_TIMEOUT_SECS && !timeout_emitted {
                let timeout_msg = format!(
                    "Failed to connect to ComfyUI after {} seconds. Please check the logs below.",
                    PORT_POLL_TIMEOUT_SECS
                );
                crate::logging::log_info(&app_handle_poll, &timeout_msg);
                let _ = app_handle_poll.emit("comfyui-timeout", timeout_msg);
                timeout_emitted = true;
            } else if count > 0 && count % 300 == 0 {
                crate::logging::log_info(
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

    Ok(child)
}

#[tauri::command]
pub fn start_comfyui(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<ComfyUiState>();
    let mut lock = match state.child.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(ref existing_child) = *lock {
        return Err(format!(
            "ComfyUI is already running (PID {})",
            existing_child.id()
        ));
    }
    let child = spawn_comfyui_process(&app_handle)?;
    *lock = Some(child);
    Ok(())
}
