// PURPOSE: Process spawner infrastructure — implements ProcessPort with I/O lifecycle.

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::time::Duration;

use std::path::PathBuf;

use launcher_shared::contract_process_port::{ProcessPort, SpawnParams};
use launcher_shared::{LogMessage, ProcessError};

fn get_rocm_path() -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
    paths.insert(0, std::path::PathBuf::from("/opt/rocm/bin"));
    paths.insert(0, std::path::PathBuf::from("/opt/rocm-7.2.4/bin"));
    std::env::join_paths(paths).unwrap_or(path)
}

fn get_comfyui_cache_env() -> (PathBuf, PathBuf, PathBuf) {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let cwd = std::env::current_dir().unwrap_or_default();
    let base = match home {
        Some(ref h) => h.join(".cache").join("comfyui-desktop"),
        None => cwd.join(".cache").join("comfyui-desktop"),
    };

    let _ = std::fs::create_dir_all(&base);
    let _ = std::fs::create_dir_all(base.join("hip"));
    let _ = std::fs::create_dir_all(base.join("comgr"));
    let _ = std::fs::create_dir_all(base.join("miopen"));
    let _ = std::fs::create_dir_all(base.join("triton"));

    let pycache = match home {
        Some(ref h) => h.join(".cache").join("comfyui-desktop").join("pycache"),
        None => base.join("pycache"),
    };
    let _ = std::fs::create_dir_all(&pycache);

    (base.clone(), base.join("hip"), pycache)
}

pub struct ProcessSpawner;

impl ProcessPort for ProcessSpawner {
    fn spawn(&self, params: SpawnParams) -> Result<(), ProcessError> {
        let new_path = get_rocm_path();

        let mut cmd = std::process::Command::new(params.python_path);
        cmd.arg("main.py")
            .arg("--extra-model-paths-config")
            .arg(params.extra_model_paths);

        if let Some(ref out_dir) = params.output_dir {
            cmd.arg("--output-directory").arg(out_dir);
        }
        if let Some(ref in_dir) = params.input_dir {
            cmd.arg("--input-directory").arg(in_dir);
        }
        if let Some(ref u_dir) = params.user_dir {
            cmd.arg("--user-directory").arg(u_dir);
        }

        // Detect VRAM size of the selected GPU using rocm-smi
        let gpu_str = params.gpu_index.as_str();
        let mut vram_bytes = None;
        let output = std::process::Command::new("rocm-smi")
            .env("PATH", &new_path)
            .arg("--showmeminfo")
            .arg("vram")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let target_gpu_header = format!("GPU[{}]", gpu_str);
                let mut found_gpu = false;
                for line in stdout.lines() {
                    if line.contains(&target_gpu_header) {
                        found_gpu = true;
                    }
                    if found_gpu && line.contains("VRAM Total Memory") {
                        if let Some(col) = line.rfind(':') {
                            if let Ok(vram) = line[col + 1..].trim().parse::<u64>() {
                                vram_bytes = Some(vram);
                                break;
                            }
                        }
                    }
                    if found_gpu && line.contains("GPU[") && !line.contains(&target_gpu_header) {
                        break;
                    }
                }
            }
        }

        // Add appropriate VRAM flags
        // Note: --normalvram was removed in ComfyUI v0.25 — default (no flag) is dynamic VRAM.
        if let Some(bytes) = vram_bytes {
            if bytes >= 12_000_000_000 {
                cmd.arg("--highvram");
            } else if bytes < 6_000_000_000 {
                cmd.arg("--lowvram");
            }
            // else: no flag = dynamic VRAM (replaces old --normalvram)
        }

        // Cache models in RAM for faster startup (28GB+ RAM recommended)
        cmd.arg("--high-ram");

        // Persistent ROCm/Torch/Triton cache — kept outside the ComfyUI project
        // so it survives restarts, reinstall, and backend changes.
        let (cache_base, hip_cache, pycache) = get_comfyui_cache_env();

        cmd.current_dir(params.comfyui_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("PATH", &new_path)
            .env("HIP_VISIBLE_DEVICES", params.gpu_index.as_str())
            .env("XDG_CACHE_HOME", cache_base)
            .env("HIP_CACHE_DIR", hip_cache)
            .env("PYTHONPYCACHEPREFIX", pycache)
            // ROCm/HIP logging — helps distinguish hang vs stuck
            .env("HIP_LOG_LEVEL", "1")
            .env("AMD_LOG_LEVEL", "4")
            .env("ROCR_DEBUG", "0")
            .process_group(0);

        if let Some(hsa_ver) = params.hsa_override {
            cmd.env("HSA_OVERRIDE_GFX_VERSION", hsa_ver.as_str());
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ProcessError::SpawnFailed(e.to_string()))?;

        // Spawn stdout reader thread
        let tx = params.log_tx.clone();
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for l in reader.lines().map_while(Result::ok) {
                    if tx.send(LogMessage::Stdout(l)).is_err() {
                        break;
                    }
                }
            });
        }

        // Spawn stderr reader thread
        let tx_stderr = params.log_tx;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for l in reader.lines().map_while(Result::ok) {
                    if tx_stderr.send(LogMessage::Stderr(l)).is_err() {
                        break;
                    }
                }
            });
        }

        // Spawn exit polling thread
        std::thread::spawn(move || loop {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => {
                    (params.on_exit)();
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        });

        Ok(())
    }
}

impl ProcessSpawner {
    /// Kill the process group by PID. Uses SIGTERM then SIGKILL.
    pub fn kill_process_group(pid: u32) {
        let pid = match i32::try_from(pid) {
            Ok(p) if p > 0 => p,
            _ => {
                eprintln!("Invalid PID {pid} for process group kill");
                return;
            }
        };
        println!("Terminating ComfyUI process group: {pid}");
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        std::thread::sleep(Duration::from_millis(500));
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }

    /// Wait for process to exit with timeout.
    pub fn wait_for_exit(child: &mut Child, timeout_secs: u64) -> Option<i32> {
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) => {
                    if start.elapsed() >= Duration::from_secs(timeout_secs) {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return None,
            }
        }
    }
}
