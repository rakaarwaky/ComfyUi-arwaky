// PURPOSE: downloader-dl — capabilities: download engine + download logic
// Implements DownloadProtocol. Delegates actual HTTP/IO to infrastructure.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use downloader_shared::contract_download_protocol::DownloadProtocol;
use downloader_shared::taxonomy_config_vo::Config;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

pub struct DownloadEngine;

impl DownloadProtocol for DownloadEngine {
    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String> {
        let token = std::env::var("HF_TOKEN").ok();
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(Duration::from_secs(15)))
                .timeout_recv_body(Some(Duration::from_secs(3600)))
                .timeout_global(Some(Duration::from_secs(3600)))
                .build(),
        );

        // Download to .tmp file first, then rename on success
        let tmp_path = dest.with_extension("tmp");
        if let Some(parent) = tmp_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        // Check existing partial download for resume
        let existing = fs::metadata(&tmp_path).ok().map(|m| m.len()).unwrap_or(0);

        let mut req = agent.get(url).header("User-Agent", "Mozilla/5.0");
        if let Some(ref t) = token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        if existing > 0 {
            req = req.header("Range", &format!("bytes={existing}-"));
        }

        let resp = match req.call() {
            Ok(resp) => resp,
            Err(ureq::Error::StatusCode(sc)) => {
                return Err(match sc {
                    401 => "Unauthorized (check HF_TOKEN)".into(),
                    403 => "Forbidden".into(),
                    404 => "Not Found — model URL may be invalid or expired".into(),
                    _ => format!("HTTP {sc}"),
                });
            }
            Err(e) => return Err(format!("Connection failed: {e}")),
        };

        let sc = resp.status().as_u16();
        let is_resume = existing > 0 && sc == 206;
        if sc != 200 && sc != 206 {
            return Err(match sc {
                401 => "Unauthorized (check HF_TOKEN)".into(),
                403 => "Forbidden".into(),
                404 => "Not Found — model URL may be invalid or expired".into(),
                _ => format!("HTTP {sc}"),
            });
        }

        let mut reader = resp.into_body().into_reader();
        let mut file = if is_resume {
            fs::OpenOptions::new()
                .append(true)
                .open(&tmp_path)
                .map_err(|e| format!("Failed to open partial file: {e}"))?
        } else {
            fs::File::create(&tmp_path).map_err(|e| format!("Failed to create file: {e}"))?
        };

        let mut buf = vec![0u8; 256 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => file
                    .write_all(&buf[..n])
                    .map_err(|e| format!("Write error: {e}"))?,
                Err(e) => return Err(format!("Read error: {e}")),
            }
        }
        drop(file);

        // Rename .tmp → final destination atomically
        fs::rename(&tmp_path, dest).map_err(|e| format!("Failed to move file: {e}"))?;
        Ok(())
    }

    fn download_one_model(
        &self,
        worker_id: usize,
        model: &Model,
        config: &Config,
        cancel_token: &Arc<AtomicBool>,
        tx: &Sender<DownloadEvent>,
    ) -> Result<(), String> {
        download_one_model(worker_id, model, config, cancel_token, tx)
    }
}

pub fn download_diffusers_bg(diffusers_dir: &Path) -> Result<(), String> {
    if diffusers_dir.exists() {
        if let Ok(entries) = fs::read_dir(diffusers_dir) {
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

/// Full download logic: direct download with progress. No HEAD probe.
pub fn download_one_model(
    worker_id: usize,
    model: &Model,
    config: &Config,
    cancel_token: &Arc<AtomicBool>,
    tx: &Sender<DownloadEvent>,
) -> Result<(), String> {
    use downloader_file_utils::infrastructure_cache_adapter::SIZE_CACHE;
    use downloader_file_utils::infrastructure_fs_adapter as fs_adapter;

    let sanitized = fs_adapter::sanitize_filename(&model.filename);
    let dest_dir = config.resolve_category_dir(&model.category);
    let dest_path = dest_dir.join(&sanitized);

    if model.category == "diffusers" {
        return download_diffusers_bg(&dest_path);
    }

    // Download directly to final location via .tmp, with resume support
    let tmp_path = dest_path.with_extension("tmp");
    if let Some(parent) = tmp_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }

    let existing = fs::metadata(&tmp_path).ok().map(|m| m.len()).unwrap_or(0);

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .timeout_global(Some(Duration::from_secs(3600)))
            .build(),
    );
    let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());

    let mut req = agent.get(&model.url).header("User-Agent", "Mozilla/5.0");
    if let Some(ref t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    if existing > 0 {
        req = req.header("Range", &format!("bytes={existing}-"));
    }

    let resp = match req.call() {
        Ok(resp) => resp,
        Err(ureq::Error::StatusCode(sc)) => {
            return Err(match sc {
                401 => "Unauthorized (check HF_TOKEN)".into(),
                403 => "Forbidden".into(),
                404 => "Not Found — model URL may be invalid or expired".into(),
                _ => format!("HTTP {sc}"),
            });
        }
        Err(e) => return Err(format!("Connection failed: {e}")),
    };

    let sc = resp.status().as_u16();
    let is_resume = existing > 0 && sc == 206;
    if sc != 200 && sc != 206 {
        return Err(match sc {
            401 => "Unauthorized (check HF_TOKEN)".into(),
            403 => "Forbidden".into(),
            404 => "Not Found — model URL may be invalid or expired".into(),
            _ => format!("HTTP {sc}"),
        });
    }

    // Determine total size from Content-Length or Content-Range
    let response_len: u64 = resp
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let total = if is_resume {
        existing + response_len
    } else if response_len > 0 {
        response_len
    } else {
        model.size_bytes
    };

    let mut reader = resp.into_body().into_reader();
    let mut file = if is_resume {
        fs::OpenOptions::new()
            .append(true)
            .open(&tmp_path)
            .map_err(|e| format!("Failed to open partial file: {e}"))?
    } else {
        fs::File::create(&tmp_path).map_err(|e| format!("Failed to create file: {e}"))?
    };

    let mut downloaded: u64 = if is_resume { existing } else { 0 };
    let start = std::time::Instant::now();
    let mut last_report = std::time::Instant::now();
    let mut buf = vec![0u8; 256 * 1024];

    loop {
        if cancel_token.load(Ordering::Acquire) {
            let _ = fs::remove_file(&tmp_path);
            return Err("Cancelled".into());
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buf[..n])
                    .map_err(|e| format!("Write error: {e}"))?;
                downloaded += n as u64;
                if last_report.elapsed() >= std::time::Duration::from_millis(200) {
                    let elapsed = start.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (downloaded.saturating_sub(if is_resume { existing } else { 0 })) as f64
                            / (1024.0 * 1024.0)
                            / elapsed
                    } else {
                        0.0
                    };
                    let eta = if speed > 0.0 && total > 0 {
                        ((total.saturating_sub(downloaded)) as f64 / (1024.0 * 1024.0) / speed)
                            as u64
                    } else {
                        0
                    };
                    let _ = tx.send(DownloadEvent::Progress {
                        worker_id,
                        filename: model.filename.clone(),
                        downloaded,
                        total,
                        speed_mb_s: speed,
                        eta_secs: eta,
                    });
                    last_report = std::time::Instant::now();
                }
            }
            Err(e) => return Err(format!("Read error: {e}")),
        }
    }
    drop(file);

    // SHA256 — wajib kalau model punya hash
    if let Some(ref expected) = model.sha256 {
        if !fs_adapter::verify_sha256(&tmp_path, expected) {
            let _ = fs::remove_file(&tmp_path);
            return Err("SHA256 mismatch — download corrupted or incomplete".into());
        }
    }

    // Rename .tmp → final destination
    fs::rename(&tmp_path, &dest_path).map_err(|e| format!("Failed to move file: {e}"))?;

    if let Ok(m) = fs::metadata(&dest_path) {
        if let Ok(mut cache) = SIZE_CACHE.write() {
            cache.sizes.insert(model.url.clone(), m.len());
            cache.save();
        }
    }
    Ok(())
}

pub fn model_sort_size(model: &Model, cache_sizes: &HashMap<String, u64>) -> u64 {
    let s = if model.size_bytes > 0 {
        model.size_bytes
    } else {
        cache_sizes.get(&model.url).copied().unwrap_or(0)
    };
    if s == 0 {
        u64::MAX
    } else {
        s
    }
}

/// HTTP HEAD refresh — pure capability (uses ureq directly, could move to infra later)
pub fn refresh_model_sizes(models: &[(usize, Model)], config: &Config) -> (usize, usize, usize) {
    use downloader_file_utils::infrastructure_cache_adapter::SIZE_CACHE;
    let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );
    let mut valid = 0usize;
    let mut invalid = 0usize;
    let mut unknown = 0usize;
    for (_idx, m) in models {
        let mut req = agent.head(&m.url).header("User-Agent", "Mozilla/5.0");
        if let Some(ref t) = token {
            req = req.header("Authorization", &format!("Bearer {t}"));
        }
        match req.call() {
            Ok(res) => {
                let status = res.status().as_u16();
                if status == 200 || status == 206 {
                    let len: u64 = res
                        .headers()
                        .get("Content-Length")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    if len > 0 {
                        if let Ok(mut cache) = SIZE_CACHE.write() {
                            cache.sizes.insert(m.url.clone(), len);
                        }
                        valid += 1;
                    } else {
                        unknown += 1;
                    }
                } else {
                    invalid += 1;
                }
            }
            Err(_) => {
                invalid += 1;
            }
        }
    }
    (valid, invalid, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_model(size: u64, url: &str) -> Model {
        Model {
            category: "test".to_string(),
            filename: "model.safetensors".to_string(),
            url: url.to_string(),
            size_bytes: size,
            sha256: None,
            group: "test-group".to_string(),
            notes: String::new(),
        }
    }

    #[test]
    fn model_sort_size_uses_model_size_when_positive() {
        let m = sample_model(1024, "https://example.com/model");
        let cache = HashMap::new();
        assert_eq!(model_sort_size(&m, &cache), 1024);
    }

    #[test]
    fn model_sort_size_uses_cache_when_model_size_zero() {
        let m = sample_model(0, "https://example.com/model");
        let mut cache = HashMap::new();
        cache.insert("https://example.com/model".to_string(), 2048);
        assert_eq!(model_sort_size(&m, &cache), 2048);
    }

    #[test]
    fn model_sort_size_returns_max_for_unknown_size() {
        let m = sample_model(0, "https://example.com/model");
        let cache = HashMap::new();
        assert_eq!(model_sort_size(&m, &cache), u64::MAX);
    }

    #[test]
    fn model_sort_size_prefers_model_size_over_cache() {
        let m = sample_model(4096, "https://example.com/model");
        let mut cache = HashMap::new();
        cache.insert("https://example.com/model".to_string(), 2048);
        assert_eq!(model_sort_size(&m, &cache), 4096);
    }
}
