use ratatui::widgets::ListState;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::downloader::{download_diffusers_bg, DownloadEvent};
use crate::model::Model;
use crate::utils::{file_exists_valid, format_size, get_available_space};

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveDownload {
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed_mb_s: f64,
    pub eta_secs: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

#[derive(Clone, PartialEq)]
pub enum AppState {
    Menu,
    Settings {
        active_field: usize, // 0 = models_dir, 1 = hf_token, 2 = save, 3 = cancel
        models_dir_input: String,
        hf_token_input: String,
    },
    DiskSpaceWarning {
        required: u64,
        available: u64,
    },
    Finished {
        completed: usize,
        failed: usize,
        message: String,
    },
}

pub struct App {
    pub models: Vec<Model>,
    pub config: Config,
    pub list_state: ListState,
    pub selected_indices: Vec<usize>, // original indices in self.models
    pub state: AppState,
    pub rx: Option<Receiver<DownloadEvent>>,
    pub cancel_token: Arc<AtomicBool>,

    // Search Mode
    pub input_mode: InputMode,
    pub search_query: String,

    // TUI UX enhancements
    pub logs: Vec<String>,
    pub active_tab: usize,
    pub tab_offset: usize,
    pub categories: Vec<String>,

    // Active downloads tracking
    pub active_downloads: Vec<Option<ActiveDownload>>,
    pub completed_count: usize,
    pub failed_count: usize,
    pub total_to_download: usize,
}

impl App {
    pub fn new(models: Vec<Model>, config: Config) -> Self {
        let mut list_state = ListState::default();
        if !models.is_empty() {
            list_state.select(Some(0));
        }

        let mut cats: Vec<String> = models.iter().map(|m| m.category.clone()).collect();
        cats.sort();
        cats.dedup();

        let mut app = Self {
            models,
            config,
            list_state,
            selected_indices: Vec::new(),
            state: AppState::Menu,
            rx: None,
            cancel_token: Arc::new(AtomicBool::new(false)),
            input_mode: InputMode::Normal,
            search_query: String::new(),
            logs: Vec::new(),
            active_tab: 0,
            tab_offset: 0,
            categories: cats,
            active_downloads: vec![None; 2],
            completed_count: 0,
            failed_count: 0,
            total_to_download: 0,
        };
        app.add_log("Downloader initialized successfully.");
        app
    }

    pub fn add_log(&mut self, msg: &str) {
        let timestamp = get_time_str();
        let log_line = format!("[{}] {}", timestamp, msg);
        self.logs.push(log_line.clone());
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }

        if let Some(path) = crate::utils::SizeCache::cache_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
                let log_file_path = parent.join("downloader.log");
                if let Ok(mut file) = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_file_path)
                {
                    let _ = writeln!(file, "{}", log_line);
                }
            }
        }
    }

    pub fn save_config_to_file(&self) -> std::io::Result<()> {
        let content = serde_yaml::to_string(&self.config).map_err(std::io::Error::other)?;

        // Save to local config.yaml
        let _ = fs::write("config.yaml", &content);

        // Save to ~/.config/comfyui-downloader/config.yaml
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(&home).join(".config/comfyui-downloader/config.yaml");
            let _ = fs::write(path, &content);
        }
        Ok(())
    }

    pub fn filtered_models(&self) -> Vec<(usize, Model)> {
        self.models
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                // 1. Filter by Tab selection
                let match_tab = match self.active_tab {
                    0 => true, // All
                    1 => {
                        // Installed
                        let dest_dir = self.config.resolve_category_dir(&m.category);
                        let dest_path = dest_dir.join(&m.filename);
                        file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                    }
                    2 => {
                        // Missing
                        let dest_dir = self.config.resolve_category_dir(&m.category);
                        let dest_path = dest_dir.join(&m.filename);
                        !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                    }
                    _ => {
                        // Category index
                        let cat_idx = self.active_tab - 3;
                        if cat_idx < self.categories.len() {
                            m.category.eq_ignore_ascii_case(&self.categories[cat_idx])
                        } else {
                            true
                        }
                    }
                };

                if !match_tab {
                    return false;
                }

                // 2. Filter by search input
                if self.search_query.is_empty() {
                    true
                } else {
                    let query = self.search_query.to_lowercase();
                    m.filename.to_lowercase().contains(&query)
                        || m.category.to_lowercase().contains(&query)
                        || m.group.to_lowercase().contains(&query)
                }
            })
            .map(|(idx, m)| (idx, m.clone()))
            .collect()
    }

    pub fn toggle_selection(&mut self) {
        let filtered = self.filtered_models();
        if let Some(selected) = self.list_state.selected() {
            if selected < filtered.len() {
                let orig_idx = filtered[selected].0;
                if let Some(pos) = self.selected_indices.iter().position(|&x| x == orig_idx) {
                    self.selected_indices.remove(pos);
                    self.add_log(&format!("Deselected: {}", self.models[orig_idx].filename));
                } else {
                    self.selected_indices.push(orig_idx);
                    self.add_log(&format!("Selected: {}", self.models[orig_idx].filename));
                }
            }
        }
    }

    pub fn select_group(&mut self, group: Option<&str>) {
        self.selected_indices.clear();
        for (i, m) in self.models.iter().enumerate() {
            let dest_dir = self.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));
            if !exists {
                if let Some(grp) = group {
                    if m.group == grp {
                        self.selected_indices.push(i);
                    }
                } else {
                    self.selected_indices.push(i); // select all missing
                }
            }
        }
        let grp_name = group.unwrap_or("All Missing");
        self.add_log(&format!(
            "Bulk selected group '{}' ({} items selected).",
            grp_name,
            self.selected_indices.len()
        ));
    }

    pub fn select_all_missing_in_view(&mut self) {
        let filtered = self.filtered_models();
        let mut count = 0;
        for (orig_idx, m) in filtered {
            let dest_dir = self.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            if !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                && !self.selected_indices.contains(&orig_idx)
            {
                self.selected_indices.push(orig_idx);
                count += 1;
            }
        }
        self.add_log(&format!(
            "Selected {} missing models in current view.",
            count
        ));
    }

    pub fn check_space_and_start(&mut self) {
        if self.rx.is_some() {
            self.add_log("Download skipped: a download or refresh task is already running.");
            return;
        }
        if self.selected_indices.is_empty() {
            self.add_log("No models selected to download.");
            return;
        }

        let mut required_space = 0;
        for &idx in &self.selected_indices {
            let m = &self.models[idx];
            let dest_dir = self.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            if !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url)) {
                required_space += m.size_bytes;
            }
        }

        let models_dir_path = PathBuf::from(&self.config.models_dir);
        let available_space = get_available_space(&models_dir_path).unwrap_or(u64::MAX);

        if available_space < required_space {
            self.add_log("Disk space pre-check warning triggered.");
            self.state = AppState::DiskSpaceWarning {
                required: required_space,
                available: available_space,
            };
        } else {
            self.start_download();
        }
    }

    pub fn refresh_selected_or_all_model_sizes(&mut self) {
        if self.rx.is_some() {
            self.add_log("Refresh skipped: download/refresh queue is already running.");
            return;
        }

        let indices: Vec<usize> = if self.selected_indices.is_empty() {
            (0..self.models.len()).collect()
        } else {
            self.selected_indices.clone()
        };

        let models_to_refresh: Vec<(usize, Model)> = indices
            .iter()
            .filter_map(|&idx| self.models.get(idx).map(|m| (idx, m.clone())))
            .collect();

        if models_to_refresh.is_empty() {
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        let config = self.config.clone();

        self.add_log(&format!(
            "Refreshing metadata for {} models...",
            models_to_refresh.len()
        ));

        std::thread::spawn(move || {
            let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
            let agent = ureq::Agent::new_with_config(
                ureq::config::Config::builder()
                    .timeout_connect(Some(std::time::Duration::from_secs(10)))
                    .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
                    .timeout_global(Some(std::time::Duration::from_secs(15)))
                    .build(),
            );

            let mut valid = 0usize;
            let mut invalid = 0usize;
            let mut unknown_size = 0usize;

            for (idx, model) in models_to_refresh {
                let mut size = 0u64;
                let mut success = false;

                // Try HEAD first
                let mut req = agent.head(&model.url).header("User-Agent", "Mozilla/5.0");
                if let Some(ref t) = token {
                    req = req.header("Authorization", &format!("Bearer {}", t));
                }

                if let Ok(res) = req.call() {
                    let status = res.status().as_u16();
                    if status == 200 || status == 206 {
                        size = res
                            .headers()
                            .get("Content-Length")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(0);
                        if size > 0 {
                            success = true;
                        }
                    }
                }

                // If HEAD failed or size is 0, try a small GET (some CDNs/gateways require this)
                if !success {
                    let mut req = agent
                        .get(&model.url)
                        .header("User-Agent", "Mozilla/5.0")
                        .header("Range", "bytes=0-0");
                    if let Some(ref t) = token {
                        req = req.header("Authorization", &format!("Bearer {}", t));
                    }
                    if let Ok(res) = req.call() {
                        let status = res.status().as_u16();
                        if status == 200 || status == 206 {
                            // If it's 206, look for Content-Range
                            if let Some(range) = res
                                .headers()
                                .get("Content-Range")
                                .and_then(|v| v.to_str().ok())
                            {
                                if let Some(total) =
                                    range.rsplit('/').next().and_then(|s| s.parse::<u64>().ok())
                                {
                                    size = total;
                                    success = true;
                                }
                            }
                            // Fallback to Content-Length if it was a 200 (though we asked for range)
                            if !success {
                                size = res
                                    .headers()
                                    .get("Content-Length")
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|v| v.parse::<u64>().ok())
                                    .unwrap_or(0);
                                if size > 0 {
                                    success = true;
                                }
                            }
                        }
                    }
                }

                if success && size > 0 {
                    let _ = tx.send(DownloadEvent::RefreshUpdate { idx, size });
                    valid += 1;
                } else if success {
                    unknown_size += 1;
                } else {
                    invalid += 1;
                }
            }

            let _ = tx.send(DownloadEvent::RefreshFinished {
                valid,
                invalid,
                unknown: unknown_size,
            });
        });
    }

    pub fn start_download(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }

        self.cancel_token.store(false, Ordering::Release);
        let (tx, rx) = channel();
        self.rx = Some(rx);

        let cache_sizes = crate::utils::SIZE_CACHE
            .read()
            .ok()
            .map(|cache| cache.sizes.clone())
            .unwrap_or_default();

        let mut selected_models: Vec<(usize, Model)> = self
            .selected_indices
            .iter()
            .map(|&idx| (idx, self.models[idx].clone()))
            .collect();

        selected_models.sort_by(|(_, left), (_, right)| {
            let left_size = model_sort_size(left, &cache_sizes);
            let right_size = model_sort_size(right, &cache_sizes);
            left_size
                .cmp(&right_size)
                .then_with(|| left.filename.cmp(&right.filename))
        });

        self.add_log("Download queue sorted smallest-to-largest by model size.");

        let config = self.config.clone();
        let cancel_token = self.cancel_token.clone();
        let total_selected = selected_models.len();

        self.add_log(&format!(
            "Starting download task queue of {} items.",
            total_selected
        ));

        self.active_downloads = vec![None; 4]; // 4 workers
        self.completed_count = 0;
        self.failed_count = 0;
        self.total_to_download = total_selected;

        // Coordinator thread
        std::thread::spawn(move || {
            let queue = Arc::new(std::sync::Mutex::new(selected_models));
            let completed_lock = Arc::new(std::sync::Mutex::new(0));
            let failed_lock = Arc::new(std::sync::Mutex::new(0));
            let mut workers = Vec::new();
            const N_WORKERS: usize = 4;

            for worker_id in 0..N_WORKERS {
                let queue = queue.clone();
                let config = config.clone();
                let cancel_token = cancel_token.clone();
                let tx = tx.clone();
                let completed_lock = completed_lock.clone();
                let failed_lock = failed_lock.clone();

                let handle = std::thread::spawn(move || loop {
                    let next_item = {
                        let mut lock = queue.lock().unwrap();
                        if lock.is_empty() {
                            None
                        } else {
                            Some(lock.remove(0))
                        }
                    };

                    let Some((_orig_idx, model)) = next_item else {
                        break;
                    };

                    if cancel_token.load(Ordering::Acquire) {
                        break;
                    }

                    let _ = tx.send(DownloadEvent::Start {
                        worker_id,
                        filename: model.filename.clone(),
                    });

                    let mut success = false;
                    let mut last_error = None;
                    for attempt in 1..=3 {
                        if cancel_token.load(Ordering::Acquire) {
                            break;
                        }

                        match download_one_model(worker_id, &model, &config, &cancel_token, &tx) {
                            Ok(_) => {
                                success = true;
                                last_error = None;
                                break;
                            }
                            Err(e) => {
                                last_error = Some(e);
                                if cancel_token.load(Ordering::Acquire) {
                                    break;
                                }
                                std::thread::sleep(Duration::from_secs(attempt * 2));
                            }
                        }
                    }

                    let _ = tx.send(DownloadEvent::ModelFinished {
                        worker_id,
                        filename: model.filename.clone(),
                        success,
                        error_msg: last_error,
                    });

                    if success {
                        let mut c = completed_lock.lock().unwrap();
                        *c += 1;
                    } else {
                        let mut f = failed_lock.lock().unwrap();
                        *f += 1;
                    }
                });
                workers.push(handle);
            }

            for w in workers {
                let _ = w.join();
            }

            let completed = *completed_lock.lock().unwrap();
            let failed = *failed_lock.lock().unwrap();
            let _ = tx.send(DownloadEvent::AllComplete { completed, failed });
        });
    }

    pub fn update_downloads(&mut self) {
        let mut events = Vec::new();
        if let Some(ref rx) = self.rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        let mut should_clear_rx = false;
        for event in events {
            match event {
                DownloadEvent::Start {
                    worker_id,
                    filename,
                } => {
                    self.add_log(&format!(
                        "Worker #{}: Starting download for {}",
                        worker_id + 1,
                        filename
                    ));
                    if worker_id < self.active_downloads.len() {
                        self.active_downloads[worker_id] = Some(ActiveDownload {
                            filename,
                            bytes_downloaded: 0,
                            total_bytes: 0,
                            speed_mb_s: 0.0,
                            eta_secs: 0,
                        });
                    }
                }
                DownloadEvent::Progress {
                    worker_id,
                    downloaded,
                    total,
                    speed_mb_s,
                    eta_secs,
                    ..
                } => {
                    if worker_id < self.active_downloads.len() {
                        if let Some(ref mut active) = self.active_downloads[worker_id] {
                            active.bytes_downloaded = downloaded;
                            active.total_bytes = total;
                            active.speed_mb_s = speed_mb_s;
                            active.eta_secs = eta_secs;
                        }
                    }
                }
                DownloadEvent::ModelFinished {
                    worker_id,
                    filename,
                    success,
                    error_msg,
                } => {
                    if success {
                        self.add_log(&format!("Worker #{}: Finished {}", worker_id + 1, filename));
                    } else {
                        let err_suffix = error_msg
                            .as_ref()
                            .map(|e| format!(": {}", e))
                            .unwrap_or_default();
                        self.add_log(&format!(
                            "Worker #{}: Failed to download {}{}",
                            worker_id + 1,
                            filename,
                            err_suffix
                        ));
                    }
                    if worker_id < self.active_downloads.len() {
                        self.active_downloads[worker_id] = None;
                    }
                    if success {
                        self.completed_count += 1;
                    } else {
                        self.failed_count += 1;
                    }
                }
                DownloadEvent::AllComplete { completed, failed } => {
                    self.add_log(&format!(
                        "Task queue complete. Successfully finished: {}, Failed/Incomplete: {}",
                        completed, failed
                    ));
                    self.state = AppState::Finished {
                        completed,
                        failed,
                        message: format!("Finished! Completed: {}, Failed: {}", completed, failed),
                    };
                    self.selected_indices.clear();
                    should_clear_rx = true;
                }
                DownloadEvent::RefreshUpdate { idx, size } => {
                    let mut filename = String::new();
                    if let Some(m) = self.models.get_mut(idx) {
                        m.size_bytes = size;
                        filename = m.filename.clone();
                        if let Ok(mut cache) = crate::utils::SIZE_CACHE.write() {
                            cache.sizes.insert(m.url.clone(), size);
                            cache.save();
                        }
                    }
                    if !filename.is_empty() {
                        self.add_log(&format!(
                            "Refreshed size for {}: {}",
                            filename,
                            format_size(size)
                        ));
                    }
                }
                DownloadEvent::RefreshFinished {
                    valid,
                    invalid,
                    unknown,
                } => {
                    self.add_log(&format!(
                        "Refresh complete: {} valid, {} invalid, {} unknown size.",
                        valid, invalid, unknown
                    ));
                    should_clear_rx = true;
                }
            }
        }

        if should_clear_rx {
            self.rx = None;
        }
    }

    pub fn ensure_active_tab_visible(&mut self, total_pages: usize) {
        if self.active_tab < self.tab_offset {
            self.tab_offset = self.active_tab;
        } else if self.active_tab >= self.tab_offset + 10 {
            self.tab_offset = self.active_tab.saturating_sub(9);
        }
        if self.tab_offset + 10 > total_pages {
            self.tab_offset = total_pages.saturating_sub(10);
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ChunkProgress {
    total_size: u64,
    #[serde(default)]
    n_chunks: usize,
    chunk_offsets: Vec<u64>,
}

pub fn download_one_model(
    worker_id: usize,
    model: &Model,
    config: &Config,
    cancel_token: &Arc<AtomicBool>,
    tx: &std::sync::mpsc::Sender<DownloadEvent>,
) -> Result<(), String> {
    use crate::utils::{sanitize_filename, verify_sha256};

    let sanitized_filename = sanitize_filename(&model.filename);
    let dest_dir = config.resolve_category_dir(&model.category);
    let dest_path = dest_dir.join(&sanitized_filename);

    if model.category == "diffusers" {
        return download_diffusers_bg(&dest_path);
    }

    let temp_dir = PathBuf::from(&config.models_dir).join(".download_tmp");
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    let temp_path = temp_dir.join(format!("{}.tmp", sanitized_filename));

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
            .timeout_global(Some(std::time::Duration::from_secs(3600)))
            .build(),
    );
    let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());

    // 1. Try HEAD request to get total_size and check Accept-Ranges
    let mut head_req = agent.head(&model.url).header("User-Agent", "Mozilla/5.0");
    if let Some(ref t) = token {
        head_req = head_req.header("Authorization", &format!("Bearer {}", t));
    }

    let mut total_size = 0u64;
    let mut supports_ranges = false;

    if let Ok(res) = head_req.call() {
        let status = res.status().as_u16();
        if status == 200 || status == 206 {
            total_size = res
                .headers()
                .get("Content-Length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            if let Some(accept_ranges) = res
                .headers()
                .get("Accept-Ranges")
                .and_then(|v| v.to_str().ok())
            {
                if accept_ranges.to_lowercase().contains("bytes") {
                    supports_ranges = true;
                }
            }
        }
    }

    // 2. If HEAD failed or accept-ranges not found, try a small GET bytes=0-0
    if !supports_ranges || total_size == 0 {
        let mut test_req = agent
            .get(&model.url)
            .header("User-Agent", "Mozilla/5.0")
            .header("Range", "bytes=0-0");
        if let Some(ref t) = token {
            test_req = test_req.header("Authorization", &format!("Bearer {}", t));
        }
        if let Ok(res) = test_req.call() {
            let status = res.status().as_u16();
            if status == 206 {
                supports_ranges = true;
                if total_size == 0 {
                    if let Some(content_range) = res
                        .headers()
                        .get("Content-Range")
                        .and_then(|v| v.to_str().ok())
                    {
                        if let Some(total) = content_range
                            .rsplit('/')
                            .next()
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            total_size = total;
                        }
                    }
                }
            }
        }
    }

    // If still 0, fallback to Model's size if populated
    if total_size == 0 {
        total_size = model.size_bytes;
    }

    if supports_ranges && total_size > 0 {
        // Dynamic chunk count based on file size
        let n_chunks: usize = if total_size < 50 * 1024 * 1024 {
            1 // <50MB: single stream
        } else if total_size < 200 * 1024 * 1024 {
            2 // <200MB: 2 chunks
        } else if total_size < 1024 * 1024 * 1024 {
            4 // <1GB: 4 chunks
        } else {
            8 // >=1GB: 8 chunks
        };
        let progress_path = temp_path.with_extension("progress");

        let chunk_size = total_size / n_chunks as u64;
        let mut chunks = Vec::new();
        for i in 0..n_chunks {
            let start = i as u64 * chunk_size;
            let end = if i == n_chunks - 1 {
                total_size - 1
            } else {
                (i as u64 + 1) * chunk_size - 1
            };
            chunks.push((start, end));
        }

        let mut chunk_offsets = vec![0u64; n_chunks];
        if progress_path.exists() && temp_path.exists() {
            if let Ok(content) = fs::read_to_string(&progress_path) {
                if let Ok(p) = serde_json::from_str::<ChunkProgress>(&content) {
                    if p.total_size == total_size
                        && p.chunk_offsets.len() == n_chunks
                        && (p.n_chunks == n_chunks || p.n_chunks == 0)
                    {
                        chunk_offsets = p.chunk_offsets;
                    }
                }
            }
        }

        // Validate actual file size on disk matches total_size if we think we are resuming
        let actual_file_size = fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
        if actual_file_size != total_size {
            chunk_offsets = vec![0u64; n_chunks];
            let file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp_path)
                .map_err(|e| e.to_string())?;
            file.set_len(total_size).map_err(|e| e.to_string())?;
        }

        let chunk_progress_atomics: Vec<Arc<AtomicU64>> = chunk_offsets
            .iter()
            .map(|&offset| Arc::new(AtomicU64::new(offset)))
            .collect();

        let file = fs::OpenOptions::new()
            .write(true)
            .open(&temp_path)
            .map_err(|e| e.to_string())?;
        let shared_file = Arc::new(file);

        let mut handles = Vec::new();
        let thread_errors = Arc::new(Mutex::new(Vec::new()));

        // Shared agent for all chunk threads — reuses connection pool
        let shared_agent = Arc::new(ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(std::time::Duration::from_secs(15)))
                .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
                .timeout_global(Some(std::time::Duration::from_secs(3600)))
                .build(),
        ));

        for i in 0..n_chunks {
            let start = chunks[i].0;
            let end = chunks[i].1;
            let initial_offset = chunk_offsets[i];
            let start_pos = start + initial_offset;

            if start_pos > end {
                // Chunk already fully completed
                continue;
            }

            let file_clone = Arc::clone(&shared_file);
            let atomic_progress = Arc::clone(&chunk_progress_atomics[i]);
            let cancel_token_clone = Arc::clone(cancel_token);
            let url = model.url.clone();
            let token_clone = token.clone();
            let thread_errors_clone = Arc::clone(&thread_errors);
            let agent_clone = Arc::clone(&shared_agent);

            let handle = thread::spawn(move || {
                let res = (|| -> Result<(), String> {
                    let mut req = agent_clone
                        .get(&url)
                        .header("User-Agent", "Mozilla/5.0")
                        .header("Range", &format!("bytes={}-{}", start_pos, end));

                    if let Some(ref t) = token_clone {
                        req = req.header("Authorization", &format!("Bearer {}", t));
                    }

                    let response = req.call().map_err(|e| e.to_string())?;
                    let status_code = response.status().as_u16();
                    if status_code != 206 && status_code != 200 {
                        return Err(format!("HTTP Error {}", status_code));
                    }

                    let mut reader = response.into_body().into_reader();
                    let mut buf = vec![0u8; 512 * 1024];
                    let mut bytes_downloaded_this_session = 0u64;

                    loop {
                        if cancel_token_clone.load(Ordering::Acquire) {
                            return Err("Cancelled".to_string());
                        }

                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let write_pos = start_pos + bytes_downloaded_this_session;
                                file_clone
                                    .write_all_at(&buf[..n], write_pos)
                                    .map_err(|e| e.to_string())?;

                                bytes_downloaded_this_session += n as u64;
                                atomic_progress.store(
                                    initial_offset + bytes_downloaded_this_session,
                                    Ordering::Release,
                                );
                            }
                            Err(e) => return Err(e.to_string()),
                        }
                    }
                    Ok(())
                })();

                if let Err(e) = res {
                    let mut errs = thread_errors_clone.lock().unwrap();
                    errs.push(e);
                }
            });
            handles.push(handle);
        }

        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut last_save = Instant::now();
        let mut last_progress_bytes = chunk_offsets.iter().sum::<u64>();
        let mut last_progress_instant = Instant::now();

        const MAX_DOWNLOAD_SECONDS: u64 = 3600;
        const STALL_TIMEOUT_SECONDS: u64 = 60;

        let mut success = true;
        let mut final_err = None;

        loop {
            if cancel_token.load(Ordering::Acquire) {
                success = false;
                final_err = Some("Cancelled".to_string());
                break;
            }

            if start_time.elapsed() > Duration::from_secs(MAX_DOWNLOAD_SECONDS) {
                success = false;
                final_err = Some("Max download time exceeded".to_string());
                break;
            }

            // Check for errors in threads
            {
                let errs = thread_errors.lock().unwrap();
                if !errs.is_empty() {
                    success = false;
                    final_err = Some(errs[0].clone());
                    break;
                }
            }

            let downloaded: u64 = chunk_progress_atomics
                .iter()
                .map(|a| a.load(Ordering::Acquire))
                .sum();

            if downloaded > last_progress_bytes {
                last_progress_bytes = downloaded;
                last_progress_instant = Instant::now();
            } else if last_progress_instant.elapsed() > Duration::from_secs(STALL_TIMEOUT_SECONDS) {
                success = false;
                final_err = Some("Download stalled - no progress".to_string());
                break;
            }

            // Send progress update
            if last_report.elapsed() >= Duration::from_millis(200) {
                let elapsed = start_time.elapsed().as_secs_f64();
                let initial_sum: u64 = chunk_offsets.iter().sum();
                let downloaded_since_start = downloaded.saturating_sub(initial_sum);
                let speed = if elapsed > 0.0 {
                    (downloaded_since_start as f64) / (1024.0 * 1024.0) / elapsed
                } else {
                    0.0
                };
                let eta = if speed > 0.0 {
                    (((total_size.saturating_sub(downloaded)) as f64) / (1024.0 * 1024.0) / speed)
                        as u64
                } else {
                    0
                };

                let _ = tx.send(DownloadEvent::Progress {
                    worker_id,
                    filename: model.filename.clone(),
                    downloaded,
                    total: total_size,
                    speed_mb_s: speed,
                    eta_secs: eta,
                });
                last_report = Instant::now();
            }

            // Periodically save progress (every 1 second)
            if last_save.elapsed() >= Duration::from_secs(1) {
                let current_offsets: Vec<u64> = chunk_progress_atomics
                    .iter()
                    .map(|a| a.load(Ordering::Acquire))
                    .collect();
                let progress = ChunkProgress {
                    total_size,
                    n_chunks,
                    chunk_offsets: current_offsets,
                };
                if let Ok(content) = serde_json::to_string(&progress) {
                    let _ = fs::write(&progress_path, content);
                }
                last_save = Instant::now();
            }

            let all_done = handles.iter().all(|h| h.is_finished());
            if all_done {
                break;
            }

            thread::sleep(Duration::from_millis(50));
        }

        // Wait for all threads to terminate
        for handle in handles {
            let _ = handle.join();
        }

        // Check again for thread errors after join
        if success {
            let errs = thread_errors.lock().unwrap();
            if !errs.is_empty() {
                success = false;
                final_err = Some(errs[0].clone());
            }
        }

        if !success {
            return Err(final_err.unwrap_or_else(|| "Unknown download error".to_string()));
        }

        let _ = fs::remove_file(&progress_path);
    } else {
        // Fallback to single-threaded download code if server doesn't support ranges
        let mut current_size = 0;
        if temp_path.exists() {
            if let Ok(metadata) = fs::metadata(&temp_path) {
                current_size = metadata.len();
            }
        }

        let mut req = agent.get(&model.url).header("User-Agent", "Mozilla/5.0");

        if let Some(ref t) = token {
            req = req.header("Authorization", &format!("Bearer {}", t));
        }

        if current_size > 0 {
            req = req.header("Range", &format!("bytes={}-", current_size));
        }

        let response = req.call().map_err(|e| e.to_string())?;

        let status_code = response.status().as_u16();
        if status_code != 200 && status_code != 206 {
            let err_msg = match status_code {
                401 => "HTTP 401: Unauthorized (Invalid/Gated token?)".to_string(),
                403 => "HTTP 403: Forbidden (Check access rights)".to_string(),
                404 => "HTTP 404: Not Found (Invalid URL/Model doesn't exist)".to_string(),
                _ => format!("HTTP Error {}", status_code),
            };
            return Err(err_msg);
        }
        let is_partial = status_code == 206;

        let response_len: u64 = response
            .headers()
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        total_size = if is_partial {
            current_size + response_len
        } else {
            if response_len > 0 {
                response_len
            } else {
                model.size_bytes
            }
        };

        let file = if is_partial {
            fs::OpenOptions::new().append(true).open(&temp_path)
        } else {
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp_path)
        };

        let file = file.map_err(|e| e.to_string())?;
        let mut writer = std::io::BufWriter::new(file);
        let mut reader = response.into_body().into_reader();
        let mut buf = vec![0u8; 512 * 1024];
        let mut downloaded: u64 = if is_partial { current_size } else { 0 };
        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut last_progress_instant = start_time;
        const MAX_DOWNLOAD_SECONDS: u64 = 3600;
        const STALL_TIMEOUT_SECONDS: u64 = 60;

        loop {
            if cancel_token.load(Ordering::Acquire) {
                return Err("Cancelled".to_string());
            }

            if start_time.elapsed() > Duration::from_secs(MAX_DOWNLOAD_SECONDS) {
                return Err("Max download time exceeded".to_string());
            }

            if last_progress_instant.elapsed() > Duration::from_secs(STALL_TIMEOUT_SECONDS) {
                return Err("Download stalled - no progress".to_string());
            }

            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    writer.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                    downloaded += n as u64;
                    last_progress_instant = Instant::now();

                    if last_report.elapsed() >= Duration::from_millis(200) {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let downloaded_since_start = if is_partial {
                            downloaded.saturating_sub(current_size)
                        } else {
                            downloaded
                        };
                        let speed = if elapsed > 0.0 {
                            (downloaded_since_start as f64) / (1024.0 * 1024.0) / elapsed
                        } else {
                            0.0
                        };
                        let eta = if speed > 0.0 {
                            (((total_size.saturating_sub(downloaded)) as f64)
                                / (1024.0 * 1024.0)
                                / speed) as u64
                        } else {
                            0
                        };

                        let _ = tx.send(DownloadEvent::Progress {
                            worker_id,
                            filename: model.filename.clone(),
                            downloaded,
                            total: total_size,
                            speed_mb_s: speed,
                            eta_secs: eta,
                        });
                        last_report = Instant::now();
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        writer.flush().map_err(|e| e.to_string())?;
        drop(writer);

        if total_size > 0 {
            let actual_size = fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
            let diff = actual_size.abs_diff(total_size);
            let allowed_diff = (total_size / 100).min(1024 * 1024); // 1% or 1MB
            if diff > allowed_diff {
                return Err(format!(
                    "Size mismatch: expected {}, got {}",
                    total_size, actual_size
                ));
            }
        }
    }

    // SHA256 Verification
    if let Some(ref expected_hash) = model.sha256 {
        let _ = tx.send(DownloadEvent::Progress {
            worker_id,
            filename: model.filename.clone(),
            downloaded: total_size,
            total: total_size,
            speed_mb_s: 0.0,
            eta_secs: 0,
        }); // Update UI to show verification phase

        if !verify_sha256(&temp_path, expected_hash) {
            let _ = fs::remove_file(&temp_path);
            return Err("SHA256 integrity check failed!".to_string());
        }
    }

    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, &dest_path).map_err(|e| e.to_string())?;

    if let Ok(metadata) = fs::metadata(&dest_path) {
        if let Ok(mut cache) = crate::utils::SIZE_CACHE.write() {
            cache.sizes.insert(model.url.clone(), metadata.len());
            cache.save();
        }
    }

    Ok(())
}

fn model_sort_size(model: &Model, cache_sizes: &std::collections::HashMap<String, u64>) -> u64 {
    let size = if model.size_bytes > 0 {
        model.size_bytes
    } else {
        cache_sizes.get(&model.url).copied().unwrap_or(0)
    };

    // If size is 0 (unknown), treat it as u64::MAX to push it to the end of the queue
    if size == 0 {
        u64::MAX
    } else {
        size
    }
}

pub(super) fn get_time_str() -> String {
    unsafe {
        let mut raw_time: libc::time_t = 0;
        libc::time(&mut raw_time);
        let mut tm = std::mem::zeroed::<libc::tm>();
        if !libc::localtime_r(&raw_time, &mut tm).is_null() {
            format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
        } else {
            "00:00:00".to_string()
        }
    }
}
