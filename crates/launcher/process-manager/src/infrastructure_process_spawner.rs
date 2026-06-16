// PURPOSE: Process spawner infrastructure — implements ProcessPort with I/O lifecycle.

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::time::Duration;

use launcher_shared::contract_process_port::{ProcessPort, SpawnParams};
use launcher_shared::{LogMessage, ProcessError};

pub struct ProcessSpawner;

impl ProcessPort for ProcessSpawner {
    fn spawn(&self, params: SpawnParams) -> Result<(), ProcessError> {
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

        cmd.current_dir(params.comfyui_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env("HIP_VISIBLE_DEVICES", params.gpu_index.as_str())
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
        unsafe { libc::kill(-pid, libc::SIGTERM); }
        std::thread::sleep(Duration::from_millis(500));
        unsafe { libc::kill(-pid, libc::SIGKILL); }
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
