// PURPOSE: downloader-tui — surface: frontend-only actions (logs, config, event loop)
// All backend logic delegated to orchestrator via DownloaderAggregate.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use std::sync::Arc;

use downloader_dl::agent_downloader_orchestrator::DownloaderOrchestrator;
use downloader_file_utils::capabilities_file_checker::{file_exists_valid, get_available_space};
use downloader_file_utils::infrastructure_cache_adapter::SIZE_CACHE;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

use crate::surface_tui_state::{ActiveDownload, App, AppState, SESSION_LOG_PATH};

// ── Logging (frontend-only) ──

impl App {
    pub fn add_log(&mut self, msg: &str) {
        let ts = get_time_str();
        let line = format!("[{ts}] {msg}");
        self.logs.push(line.clone());
        if self.logs.len() > 100 { self.logs.remove(0); }
        if let Some(p) = SESSION_LOG_PATH.get() {
            if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(p) {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    pub fn copy_logs_to_clipboard(&self) -> String {
        let full = self.logs.join("\n");
        let ok = std::process::Command::new("sh")
            .arg("-c").arg("xclip -selection clipboard 2>/dev/null || wl-copy 2>/dev/null || true")
            .stdin(std::process::Stdio::piped())
            .spawn().and_then(|mut c| {
                if let Some(ref mut s) = c.stdin { let _ = s.write_all(full.as_bytes()); }
                c.wait()
            }).map(|s| s.success()).unwrap_or(false);
        let p = SESSION_LOG_PATH.get().map(|p| p.display().to_string()).unwrap_or_default();
        if ok { format!("✓ Logs copied ({} lines)", self.logs.len()) }
        else { format!("Log: {p} ({} lines)", self.logs.len()) }
    }
}

pub fn trace_log(msg: &str) {
    let ts = get_time_str();
    if let Some(p) = SESSION_LOG_PATH.get() {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "[{ts}] [TRACE] {msg}");
        }
    }
}

fn get_time_str() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

// ── Config (frontend-only) ──

impl App {
    pub fn save_config_to_file(&self) -> std::io::Result<()> {
        let c = serde_yaml::to_string(&self.config).map_err(std::io::Error::other)?;
        let _ = fs::write("config.yaml", &c);
        if let Ok(h) = std::env::var("HOME") {
            let _ = fs::write(PathBuf::from(&h).join(".config/comfyui-downloader/config.yaml"), &c);
        }
        Ok(())
    }
}

fn build_orch() -> DownloaderOrchestrator {
    use downloader_shared::contract_cache_port::CachePort;
    use downloader_shared::contract_config_port::ConfigPort;
    use downloader_shared::contract_download_protocol::DownloadProtocol;
    use downloader_shared::contract_file_port::FileValidationPort;
    use downloader_shared::contract_file_protocol::FileValidationProtocol;
    use downloader_config::ConfigLoader;
    use downloader_file_utils::capabilities_file_checker::FileChecker;
    use downloader_file_utils::infrastructure_cache_adapter::SizeCache;
    use downloader_file_utils::infrastructure_fs_adapter::FsAdapter;
    use downloader_dl::capabilities_download_engine::DownloadEngine;

    DownloaderOrchestrator {
        config_port: Arc::new(ConfigLoader) as Arc<dyn ConfigPort>,
        file_port: Arc::new(FsAdapter) as Arc<dyn FileValidationPort>,
        file_protocol: Arc::new(FileChecker) as Arc<dyn FileValidationProtocol>,
        cache_port: Arc::new(SizeCache::load()) as Arc<dyn CachePort>,
        download_protocol: Arc::new(DownloadEngine) as Arc<dyn DownloadProtocol>,
    }
}

// ── Check space + delegate to orchestrator ──

impl App {
    pub fn check_space_and_start(&mut self) {
        if self.rx.is_some() {
            self.add_log("A task is already running."); return;
        }
        if self.selected_indices.is_empty() {
            self.add_log("No models selected."); return;
        }
        let mut need = 0u64;
        for &i in &self.selected_indices {
            let m = &self.models[i];
            let d = self.config.resolve_category_dir(&m.category).join(&m.filename);
            if !file_exists_valid(&d, m.size_bytes, Some(&m.url)) { need += m.size_bytes; }
        }
        let avail = get_available_space(&PathBuf::from(&self.config.models_dir)).unwrap_or(u64::MAX);
        if avail < need {
            self.add_log("Disk space warning.");
            self.state = AppState::DiskSpaceWarning { required: need, available: avail };
        } else {
            self.start_download_via_orch();
        }
    }

    fn start_download_via_orch(&mut self) {
        let orch = build_orch();
        let cancel = self.cancel_token.clone();
        cancel.store(false, std::sync::atomic::Ordering::Release);
        let selected: Vec<(usize, Model)> = self.selected_indices.iter()
            .map(|&i| (i, self.models[i].clone())).collect();

        self.add_log(&format!("Starting download of {} items.", selected.len()));
        self.active_downloads = vec![None; 1];
        self.completed_count = 0; self.failed_count = 0;
        self.total_to_download = selected.len();

        let rx = orch.start_download_coordinator(selected, self.config.clone(), cancel.clone());
        self.rx = Some(rx);
    }

    pub fn refresh_sizes(&mut self) {
        if self.rx.is_some() {
            self.add_log("A task is already running."); return;
        }
        let orch = build_orch();
        let indices: Vec<usize> = if self.selected_indices.is_empty() {
            (0..self.models.len()).collect()
        } else { self.selected_indices.clone() };
        let models: Vec<(usize, Model)> = indices.iter()
            .filter_map(|&i| self.models.get(i).map(|m| (i, m.clone()))).collect();
        if models.is_empty() { return; }

        self.add_log(&format!("Refreshing {} model sizes...", models.len()));
        let rx = orch.refresh_metadata(models, self.config.clone());
        self.rx = Some(rx);
    }
}

// ── Event processing (frontend-only) ──

impl App {
    pub fn update_downloads(&mut self) {
        let mut events = Vec::new();
        if let Some(ref rx) = self.rx {
            while let Ok(e) = rx.try_recv() { events.push(e); }
        }
        let mut clear = false;
        for e in events {
            match e {
                DownloadEvent::Start { worker_id, filename } => {
                    self.add_log(&format!("W#{}: Starting {filename}", worker_id + 1));
                    if worker_id < self.active_downloads.len() {
                        self.active_downloads[worker_id] = Some(ActiveDownload {
                            filename, bytes_downloaded: 0, total_bytes: 0, speed_mb_s: 0.0, eta_secs: 0,
                        });
                    }
                }
                DownloadEvent::Progress { worker_id, downloaded, total, speed_mb_s, eta_secs, .. } => {
                    if let Some(ref mut a) = self.active_downloads.get_mut(worker_id).and_then(|o| o.as_mut()) {
                        a.bytes_downloaded = downloaded; a.total_bytes = total;
                        a.speed_mb_s = speed_mb_s; a.eta_secs = eta_secs;
                    }
                }
                DownloadEvent::ModelFinished {
                    worker_id,
                    filename,
                    success,
                    error_msg,
                } => {
                    if success {
                        self.add_log(&format!("W#{}: ✓ {filename}", worker_id + 1));
                        self.completed_count += 1;
                        self.mark_filtered_dirty();
                    } else {
                        let detail = error_msg.as_deref().unwrap_or("unknown error");
                        self.add_log(&format!("W#{}: ✗ {filename} — {detail}", worker_id + 1));
                        self.failed_count += 1;
                    }
                    if worker_id < self.active_downloads.len() { self.active_downloads[worker_id] = None; }
                }
                DownloadEvent::AllComplete { completed, failed } => {
                    let total = completed + failed;
                    self.add_log(&format!("Queue done. OK: {completed}, Fail: {failed} (of {total})"));
                    self.state = AppState::Finished {
                        completed, failed, message: format!("Done! OK: {completed}, Fail: {failed}"),
                    };
                    self.selected_indices.clear(); clear = true; self.mark_filtered_dirty();
                }
                DownloadEvent::RefreshUpdate { idx, size } => {
                    if let Some(m) = self.models.get_mut(idx) {
                        m.size_bytes = size;
                        if let Ok(mut c) = SIZE_CACHE.write() { c.sizes.insert(m.url.clone(), size); }
                    }
                }
                DownloadEvent::RefreshFinished { .. } => {
                    if let Ok(c) = SIZE_CACHE.write() { c.save(); }
                    clear = true; self.mark_filtered_dirty();
                }
            }
        }
        if clear { self.rx = None; }
    }
}
