use std::fs;
use std::path::Path;

#[derive(Clone)]
pub enum DownloadEvent {
    Start {
        worker_id: usize,
        filename: String,
    },
    Progress {
        worker_id: usize,
        filename: String,
        downloaded: u64,
        total: u64,
        speed_mb_s: f64,
        eta_secs: u64,
    },
    ModelFinished {
        worker_id: usize,
        filename: String,
        success: bool,
        error_msg: Option<String>,
    },
    AllComplete {
        completed: usize,
        failed: usize,
    },
}

pub fn download_diffusers_bg(diffusers_dir: &Path) -> Result<(), String> {
    if diffusers_dir.exists() {
        if let Ok(entries) = fs::read_dir(&diffusers_dir) {
            if entries.count() > 10 {
                return Ok(());
            }
        }
    }

    let mut cmd = std::process::Command::new("huggingface-cli");
    cmd.arg("download")
        .arg("stabilityai/stable-diffusion-xl-base-1.0")
        .arg("--local-dir")
        .arg(diffusers_dir);

    if let Ok(token) = std::env::var("HF_TOKEN") {
        cmd.arg("--token").arg(token);
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("huggingface-cli build failed".to_string())
    }
}
