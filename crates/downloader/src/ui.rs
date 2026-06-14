use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{Config, load_config};
use crate::model::{Model, get_models};
use crate::utils::{file_exists_valid, format_size, get_available_space};
use crate::downloader::{DownloadEvent, download_diffusers_bg};

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
    Downloading {
        active_downloads: Vec<Option<ActiveDownload>>, // mapped by worker_id
        completed_count: usize,
        failed_count: usize,
        total_to_download: usize,
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
}

impl App {
    pub fn new(models: Vec<Model>, config: Config) -> Self {
        let mut list_state = ListState::default();
        if !models.is_empty() {
            list_state.select(Some(0));
        }
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
        let content = serde_yaml::to_string(&self.config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
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
                    0 => true,
                    1 => m.group == "flux",
                    2 => m.group == "sdxl",
                    3 => m.group == "video",
                    4 => m.group == "other",
                    5 => {
                        let dest_dir = self.config.resolve_category_dir(&m.category);
                        let dest_path = dest_dir.join(&m.filename);
                        !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                    }
                    6 => {
                        let dest_dir = self.config.resolve_category_dir(&m.category);
                        let dest_path = dest_dir.join(&m.filename);
                        file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                    }
                    _ => true,
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
        self.add_log(&format!("Bulk selected group '{}' ({} items selected).", grp_name, self.selected_indices.len()));
    }

    pub fn select_all_missing_in_view(&mut self) {
        let filtered = self.filtered_models();
        let mut count = 0;
        for (orig_idx, m) in filtered {
            let dest_dir = self.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            if !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url)) {
                if !self.selected_indices.contains(&orig_idx) {
                    self.selected_indices.push(orig_idx);
                    count += 1;
                }
            }
        }
        self.add_log(&format!("Selected {} missing models in current view.", count));
    }

    pub fn check_space_and_start(&mut self) {
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

    pub fn start_download(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }

        self.cancel_token.store(false, Ordering::Release);
        let (tx, rx) = channel();
        self.rx = Some(rx);

        let selected_models: Vec<(usize, Model)> = self
            .selected_indices
            .iter()
            .map(|&idx| (idx, self.models[idx].clone()))
            .collect();

        let config = self.config.clone();
        let cancel_token = self.cancel_token.clone();
        let total_selected = selected_models.len();

        self.add_log(&format!("Starting download task queue of {} items.", total_selected));

        self.state = AppState::Downloading {
            active_downloads: vec![None; 2], // 2 workers
            completed_count: 0,
            failed_count: 0,
            total_to_download: total_selected,
        };

        // Coordinator thread
        std::thread::spawn(move || {
            let queue = Arc::new(std::sync::Mutex::new(selected_models));
            let completed_lock = Arc::new(std::sync::Mutex::new(0));
            let failed_lock = Arc::new(std::sync::Mutex::new(0));
            let mut workers = Vec::new();
            const N_WORKERS: usize = 2;

            for worker_id in 0..N_WORKERS {
                let queue = queue.clone();
                let config = config.clone();
                let cancel_token = cancel_token.clone();
                let tx = tx.clone();
                let completed_lock = completed_lock.clone();
                let failed_lock = failed_lock.clone();

                let handle = std::thread::spawn(move || {
                    loop {
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
                    self.add_log(&format!("Worker #{}: Starting download for {}", worker_id + 1, filename));
                    if let AppState::Downloading {
                        ref mut active_downloads,
                        ..
                    } = self.state
                    {
                        if worker_id < active_downloads.len() {
                            active_downloads[worker_id] = Some(ActiveDownload {
                                filename,
                                bytes_downloaded: 0,
                                total_bytes: 0,
                                speed_mb_s: 0.0,
                                eta_secs: 0,
                            });
                        }
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
                    if let AppState::Downloading {
                        ref mut active_downloads,
                        ..
                    } = self.state
                    {
                        if worker_id < active_downloads.len() {
                            if let Some(ref mut active) = active_downloads[worker_id] {
                                active.bytes_downloaded = downloaded;
                                active.total_bytes = total;
                                active.speed_mb_s = speed_mb_s;
                                active.eta_secs = eta_secs;
                            }
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
                        let err_suffix = error_msg.as_ref().map(|e| format!(": {}", e)).unwrap_or_default();
                        self.add_log(&format!("Worker #{}: Failed to download {}{}", worker_id + 1, filename, err_suffix));
                    }
                    if let AppState::Downloading {
                        ref mut active_downloads,
                        ref mut completed_count,
                        ref mut failed_count,
                        ..
                    } = self.state
                    {
                        if worker_id < active_downloads.len() {
                            active_downloads[worker_id] = None;
                        }
                        if success {
                            *completed_count += 1;
                        } else {
                            *failed_count += 1;
                        }
                    }
                }
                DownloadEvent::AllComplete { completed, failed } => {
                    self.add_log(&format!("Task queue complete. Successfully finished: {}, Failed/Incomplete: {}", completed, failed));
                    self.state = AppState::Finished {
                        completed,
                        failed,
                        message: format!(
                            "Finished! Completed: {}, Failed: {}",
                            completed, failed
                        ),
                    };
                    self.selected_indices.clear();
                    should_clear_rx = true;
                }
            }
        }

        if should_clear_rx {
            self.rx = None;
        }
    }
}

fn download_one_model(
    worker_id: usize,
    model: &Model,
    config: &Config,
    cancel_token: &Arc<AtomicBool>,
    tx: &std::sync::mpsc::Sender<DownloadEvent>,
) -> Result<(), String> {
    let dest_dir = config.resolve_category_dir(&model.category);
    let dest_path = dest_dir.join(&model.filename);

    if model.category == "diffusers" {
        return download_diffusers_bg(&dest_path);
    }

    let temp_dir = PathBuf::from(&config.models_dir).join(".download_tmp");
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    let temp_path = temp_dir.join(format!("{}.tmp", model.filename));

    let mut current_size = 0;
    if temp_path.exists() {
        if let Ok(metadata) = fs::metadata(&temp_path) {
            current_size = metadata.len();
        }
    }

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(std::time::Duration::from_secs(30)))
            .build(),
    );

    let mut req = agent.get(&model.url).header("User-Agent", "Mozilla/5.0");
    
    let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
    if let Some(t) = token {
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

    let total_size = if is_partial {
        current_size + response_len
    } else {
        if response_len > 0 {
            response_len
        } else {
            model.size_bytes
        }
    };

    let file = if is_partial {
        fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&temp_path)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
    };

    let file = file.map_err(|e| e.to_string())?;
    let mut writer = io::BufWriter::new(file);
    let mut reader = response.into_body().into_reader();
    let mut buf = vec![0u8; 128 * 1024];
    let mut downloaded: u64 = if is_partial { current_size } else { 0 };
    let start_time = Instant::now();
    let mut last_report = Instant::now();

    loop {
        if cancel_token.load(Ordering::Acquire) {
            return Err("Cancelled".to_string());
        }

        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                writer.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                downloaded += n as u64;

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
        let min_allowed = (total_size as f64 * 0.95) as u64;
        if actual_size < min_allowed {
            return Err("File size checks failed".to_string());
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

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    let models = get_models();

    // Spin up background size checking thread
    {
        let config = config.clone();
        let models = models.clone();
        std::thread::spawn(move || {
            let agent = ureq::Agent::new_with_config(
                ureq::config::Config::builder()
                    .timeout_connect(Some(std::time::Duration::from_secs(10)))
                    .timeout_global(Some(std::time::Duration::from_secs(10)))
                    .build(),
            );

            for m in models {
                let dest_dir = config.resolve_category_dir(&m.category);
                let dest_path = dest_dir.join(&m.filename);
                if dest_path.is_file() {
                    let needs_verification = {
                        let actual_size = fs::metadata(&dest_path).map(|meta| meta.len()).unwrap_or(0);
                        if actual_size > 0 {
                            let is_standard_valid = if m.size_bytes <= 1_000_000 {
                                actual_size >= 1000
                            } else {
                                let min_allowed = (m.size_bytes as f64 * 0.95) as u64;
                                actual_size >= min_allowed
                            };
                            
                            if !is_standard_valid || m.size_bytes == 0 {
                                let has_cached = if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                                    cache.sizes.contains_key(&m.url)
                                } else {
                                    false
                                };
                                !has_cached
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };

                    if needs_verification {
                        let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
                        let mut req = agent.head(&m.url).header("User-Agent", "Mozilla/5.0");
                        if let Some(t) = token {
                            req = req.header("Authorization", &format!("Bearer {}", t));
                        }

                        if let Ok(res) = req.call() {
                            let status = res.status().as_u16();
                            if status == 200 || status == 206 {
                                let response_len: u64 = res
                                    .headers()
                                    .get("Content-Length")
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0);
                                
                                if response_len > 0 {
                                    if let Ok(mut cache) = crate::utils::SIZE_CACHE.write() {
                                        cache.sizes.insert(m.url.clone(), response_len);
                                        cache.save();
                                    }
                                }
                            } else {
                                if let Some(path) = crate::utils::SizeCache::cache_path() {
                                    if let Some(parent) = path.parent() {
                                        let log_file_path = parent.join("downloader.log");
                                        if let Ok(mut file) = fs::OpenOptions::new()
                                            .create(true)
                                            .append(true)
                                            .open(log_file_path)
                                        {
                                            let timestamp = get_time_str();
                                            let _ = writeln!(
                                                file,
                                                "[{}] Background check: Invalid URL for {} (Status: {})",
                                                timestamp, m.filename, status
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        });
    }

    // Check CLI argument first
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--status" => {
                println!(">>> Model Collection Status <<<\n");
                for m in &models {
                    let dest_dir = config.resolve_category_dir(&m.category);
                    let dest_path = dest_dir.join(&m.filename);
                    let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));
                    let size = if m.size_bytes > 0 {
                        m.size_bytes
                    } else if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                        *cache.sizes.get(&m.url).unwrap_or(&0)
                    } else {
                        0
                    };
                    println!(
                        "{:<55} {:>12} {}",
                        format!("{}/{}", m.category, m.filename),
                        format_size(size),
                        if exists { "\x1b[32m✓ READY\x1b[0m" } else { "\x1b[31m✗ MISSING\x1b[0m" }
                    );
                }
                return Ok(());
            }
            "--recommend" | "--rx6800xt" | "--amd" => {
                println!("RX6800XT 16GB VRAM - Optimal Settings Guide\n");
                println!("FLUX Dev (Text-to-Image) GGUF Recommended: flux1-dev-Q5_K_S.gguf (~8.3 GB)");
                println!("FLUX Fill (Inpaint/Outpaint) GGUF Recommended: flux1-fill-dev-Q4_K_S.gguf (~12 GB)");
                return Ok(());
            }
            _ => {}
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(models, config);

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        app.update_downloads();

        terminal.draw(|f| draw_ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match app.state {
                        AppState::Menu => {
                            if app.input_mode == InputMode::Search {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Enter => {
                                        app.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Backspace => {
                                        app.search_query.pop();
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char(c) => {
                                        app.search_query.push(c);
                                        app.list_state.select(Some(0));
                                    }
                                    _ => {}
                                }
                            } else {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => break,
                                    KeyCode::Char('/') => {
                                        app.input_mode = InputMode::Search;
                                    }
                                    KeyCode::Char('c') => {
                                        app.state = AppState::Settings {
                                            active_field: 0,
                                            models_dir_input: app.config.models_dir.clone(),
                                            hf_token_input: app.config.hf_token.clone().unwrap_or_default(),
                                        };
                                        app.add_log("Settings menu opened.");
                                    }
                                    KeyCode::Tab => {
                                        app.active_tab = (app.active_tab + 1) % 7;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::BackTab => {
                                        app.active_tab = if app.active_tab == 0 { 6 } else { app.active_tab - 1 };
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Up => {
                                        let filtered_len = app.filtered_models().len();
                                        if filtered_len > 0 {
                                            let i = match app.list_state.selected() {
                                                Some(i) => {
                                                    if i == 0 {
                                                        filtered_len - 1
                                                    } else {
                                                        i - 1
                                                    }
                                                }
                                                None => 0,
                                            };
                                            app.list_state.select(Some(i));
                                        }
                                    }
                                    KeyCode::Down => {
                                        let filtered_len = app.filtered_models().len();
                                        if filtered_len > 0 {
                                            let i = match app.list_state.selected() {
                                                Some(i) => {
                                                    if i >= filtered_len - 1 {
                                                        0
                                                    } else {
                                                        i + 1
                                                    }
                                                }
                                                None => 0,
                                            };
                                            app.list_state.select(Some(i));
                                        }
                                    }
                                    KeyCode::Char(' ') => {
                                        app.toggle_selection();
                                    }
                                    KeyCode::Char('a') | KeyCode::Char('A') => {
                                        app.select_all_missing_in_view();
                                    }
                                    KeyCode::Char('1') => {
                                        app.active_tab = 1;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('2') => {
                                        app.active_tab = 2;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('3') => {
                                        app.active_tab = 3;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('4') => {
                                        app.active_tab = 4;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('5') => {
                                        app.active_tab = 5;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('6') => {
                                        app.active_tab = 6;
                                        app.list_state.select(Some(0));
                                    }
                                    KeyCode::Char('d') | KeyCode::Enter => {
                                        app.check_space_and_start();
                                    }
                                    _ => {}
                                }
                            }
                        }
                        AppState::Settings {
                            ref mut active_field,
                            ref mut models_dir_input,
                            ref mut hf_token_input,
                        } => match key.code {
                            KeyCode::Esc => {
                                app.state = AppState::Menu;
                                app.add_log("Settings menu closed without saving.");
                            }
                            KeyCode::Tab | KeyCode::Down => {
                                *active_field = (*active_field + 1) % 4;
                            }
                            KeyCode::BackTab | KeyCode::Up => {
                                *active_field = if *active_field == 0 { 3 } else { *active_field - 1 };
                            }
                            KeyCode::Enter => {
                                if *active_field == 2 {
                                    // Save settings
                                    app.config.models_dir = models_dir_input.clone();
                                    app.config.hf_token = if hf_token_input.is_empty() {
                                        None
                                    } else {
                                        Some(hf_token_input.clone())
                                    };
                                    if let Err(e) = app.save_config_to_file() {
                                        app.add_log(&format!("Failed to save config: {:?}", e));
                                    } else {
                                        app.add_log("Configuration saved successfully.");
                                    }
                                    app.state = AppState::Menu;
                                } else if *active_field == 3 {
                                    // Cancel settings
                                    app.state = AppState::Menu;
                                    app.add_log("Settings menu closed without saving.");
                                } else {
                                    // Enter acts as Tab inside text fields
                                    *active_field = (*active_field + 1) % 4;
                                }
                            }
                            KeyCode::Backspace => {
                                if *active_field == 0 {
                                    models_dir_input.pop();
                                } else if *active_field == 1 {
                                    hf_token_input.pop();
                                }
                            }
                            KeyCode::Char(c) => {
                                if *active_field == 0 {
                                    models_dir_input.push(c);
                                } else if *active_field == 1 {
                                    hf_token_input.push(c);
                                }
                            }
                            _ => {}
                        },
                        AppState::DiskSpaceWarning { .. } => match key.code {
                            KeyCode::Enter => {
                                app.add_log("Proceeding with download despite disk space warning.");
                                app.start_download();
                            }
                            KeyCode::Esc => {
                                app.state = AppState::Menu;
                            }
                            _ => {}
                        },
                        AppState::Downloading { .. } => {
                            if key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                app.add_log("User cancelled downloading queue.");
                                app.cancel_token.store(true, Ordering::Release);
                            }
                        }
                        AppState::Finished { .. } => {
                            if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                                app.state = AppState::Menu;
                            }
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.size();

    let title_suffix = if app.input_mode == InputMode::Search {
        format!(" [SEARCHING: {}] ", app.search_query)
    } else if !app.search_query.is_empty() {
        format!(" [Filter: {}] ", app.search_query)
    } else {
        String::new()
    };

    let outer_block = Block::default()
        .title(format!(" ComfyUI Desktop Model Downloader v2.2 (Ratatui TUI){} ", title_suffix))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(outer_block, size);

    let inner_rect = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(7), // Live Activity Log box
            Constraint::Length(3), // Status Bar Footer
        ])
        .split(size);

    // Filtered list
    let filtered = app.filtered_models();

    // Tab Navigation Bar
    let tab_titles = vec![
        " All [0] ",
        " FLUX [1] ",
        " SDXL [2] ",
        " Video [3] ",
        " Other [4] ",
        " Missing [5] ",
        " Installed [6] ",
    ];
    let tabs = ratatui::widgets::Tabs::new(
        tab_titles
            .iter()
            .map(|t| Span::raw(*t))
            .collect::<Vec<Span>>(),
    )
    .block(Block::default().borders(Borders::BOTTOM))
    .select(app.active_tab)
    .style(Style::default().fg(Color::Gray))
    .highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    );

    let body_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(5)])
        .split(inner_rect[0]);

    f.render_widget(tabs, body_layout[0]);

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(body_layout[1]);

    // Draw Left Side: Model List
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(orig_idx, m)| {
            let dest_dir = app.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));

            let prefix = if app.selected_indices.contains(orig_idx) {
                "[✔] "
            } else {
                "[ ] "
            };

            let status_span = if exists {
                Span::styled(" READY ", Style::default().bg(Color::Green).fg(Color::Black))
            } else {
                Span::styled(" MISSING ", Style::default().bg(Color::Red).fg(Color::White))
            };

            let size = if m.size_bytes > 0 {
                m.size_bytes
            } else if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                *cache.sizes.get(&m.url).unwrap_or(&0)
            } else {
                0
            };

            let text = Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    format!("{:<45}", format!("{}/{}", m.category, m.filename)),
                    Style::default().fg(if exists { Color::DarkGray } else { Color::White }),
                ),
                Span::raw(format!(" {:>10}  ", format_size(size))),
                status_span,
            ]);

            ListItem::new(text)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Model List (Space to toggle) ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 41, 59))
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, main_layout[0], &mut app.list_state);

    // Draw Right Side: Model Info & Tips
    let right_rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[1]);

    let details_text = if let Some(selected) = app.list_state.selected() {
        if selected < filtered.len() {
            let m = &filtered[selected].1;
            let dest_dir = app.config.resolve_category_dir(&m.category);
            let dest_path = dest_dir.join(&m.filename);
            let exists = file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));

            let size = if m.size_bytes > 0 {
                m.size_bytes
            } else if let Ok(cache) = crate::utils::SIZE_CACHE.read() {
                *cache.sizes.get(&m.url).unwrap_or(&0)
            } else {
                0
            };

            format!(
                "Filename: {}\nCategory: {}\nGroup: {}\nEstimated Size: {}\nStatus: {}\nNotes: {}\n\nURL: {}",
                m.filename,
                m.category,
                m.group,
                format_size(size),
                if exists { "✓ Installed" } else { "✗ Not Found" },
                m.notes,
                m.url
            )
        } else {
            "No model selected.".to_string()
        }
    } else {
        "No model selected.".to_string()
    };

    let details_paragraph = Paragraph::new(details_text)
        .block(Block::default().title(" Selected Model Info ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(details_paragraph, right_rects[0]);

    let guide_text = "RX6800XT 16GB VRAM Tips:\n\
                      - GGUF quants (Q5_K_S) are recommended for FLUX Dev.\n\
                      - FP8 quants are memory efficient.\n\
                      - Keep batch size to 1 for FLUX, max 2-3 for SDXL.\n\
                      - Set HSA_OVERRIDE_GFX_VERSION=10.3.0 in environment.";
    let guide_paragraph = Paragraph::new(guide_text)
        .block(Block::default().title(" GPU Optimization Guide ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(guide_paragraph, right_rects[1]);

    // Draw Bottom Logs
    let max_lines = 5;
    let log_start = app.logs.len().saturating_sub(max_lines);
    let logs_to_show = &app.logs[log_start..];
    let logs_text = logs_to_show.join("\n");
    let logs_paragraph = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title(" Live Activity Log ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true });
    f.render_widget(logs_paragraph, inner_rect[1]);

    // Draw Bottom Colored Status Bar Footer
    let help_text = if app.input_mode == InputMode::Search {
        "  [Type to Search]  |  [Enter/Esc] to exit search mode  |  [Backspace] to delete  "
    } else {
        "  [Tab/Shift+Tab] Cycle Tabs  |  [Space] Toggle  |  [a] Select All Missing  |  [/] Search  |  [c] Settings  |  [Enter/d] Download  |  [Esc/q] Exit  "
    };
    let footer_paragraph = Paragraph::new(Span::styled(
        help_text,
        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
    ))
    .style(Style::default().bg(Color::Cyan));
    f.render_widget(footer_paragraph, inner_rect[2]);

    // Render Overlay Popups based on state
    match app.state {
        AppState::Menu => {}
        AppState::Settings {
            active_field,
            ref models_dir_input,
            ref hf_token_input,
        } => {
            let popup_rect = centered_rect(65, 45, size);
            f.render_widget(Clear, popup_rect);

            let active_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let inactive_style = Style::default().fg(Color::Gray);

            let dir_style = if active_field == 0 { active_style } else { inactive_style };
            let token_style = if active_field == 1 { active_style } else { inactive_style };
            let save_style = if active_field == 2 { active_style } else { inactive_style };
            let cancel_style = if active_field == 3 { active_style } else { inactive_style };

            let settings_spans = vec![
                Line::from(vec![
                    Span::styled("1. Models Download Directory:", dir_style),
                ]),
                Line::from(vec![
                    Span::styled(format!("   > {} ", models_dir_input), dir_style),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("2. HuggingFace Access Token (HF_TOKEN):", token_style),
                ]),
                Line::from(vec![
                    Span::styled(format!("   > {} ", hf_token_input), token_style),
                ]),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled("  [ SAVE ]  ", save_style),
                    Span::raw("      "),
                    Span::styled("  [ CANCEL ]  ", cancel_style),
                ]),
                Line::from(""),
                Line::from(Span::styled("Use [Tab] or [Up/Down] to navigate. Type to edit path/token. Save updates config.yaml.", Style::default().fg(Color::DarkGray))),
            ];

            let settings_paragraph = Paragraph::new(settings_spans)
                .block(
                    Block::default()
                        .title(" Settings & Configuration ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(settings_paragraph, popup_rect);
        }
        AppState::DiskSpaceWarning { required, available } => {
            let popup_rect = centered_rect(65, 30, size);
            f.render_widget(Clear, popup_rect);

            let warning_text = format!(
                "⚠️ INSUFFICIENT DISK SPACE WARNING ⚠️\n\n\
                 Available Space: {}\n\
                 Total Required: {}\n\n\
                 The download destination directory partition is running low.\n\n\
                 Press [Enter] to ignore and proceed anyway.\n\
                 Press [Esc] to return and change your model selections.",
                format_size(available),
                format_size(required)
            );

            let warning_paragraph = Paragraph::new(warning_text)
                .block(
                    Block::default()
                        .title(" Disk Space Pre-Check ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(warning_paragraph, popup_rect);
        }
        AppState::Downloading {
            ref active_downloads,
            completed_count,
            failed_count,
            total_to_download,
        } => {
            let popup_rect = centered_rect(70, 42, size);
            f.render_widget(Clear, popup_rect);

            let mut progress_text = format!(
                "Overall Progress: Completed: {} | Failed: {} | Remaining Tasks: {}\n\
                 Workers: {} Active\n\n",
                completed_count,
                failed_count,
                total_to_download.saturating_sub(completed_count + failed_count),
                active_downloads.iter().filter(|x| x.is_some()).count()
            );

            for (w_id, active) in active_downloads.iter().enumerate() {
                progress_text.push_str(&format!("  Worker #{}: ", w_id + 1));
                if let Some(dl) = active {
                    let pct = if dl.total_bytes > 0 {
                        (dl.bytes_downloaded as f64 / dl.total_bytes as f64 * 100.0) as u16
                    } else {
                        0
                    };
                    let bar_width = 25;
                    let bar = draw_progress_bar(pct, bar_width);
                    progress_text.push_str(&format!(
                        "{} {}\n    Progress: {}/{} | Speed: {:.2} MB/s | ETA: {}s\n\n",
                        dl.filename,
                        bar,
                        format_size(dl.bytes_downloaded),
                        format_size(dl.total_bytes),
                        dl.speed_mb_s,
                        dl.eta_secs
                    ));
                } else {
                    progress_text.push_str("Idle / Waiting for task...\n\n");
                }
            }

            progress_text.push_str("Press [Ctrl + C] to cancel all downloads");

            let progress_paragraph = Paragraph::new(progress_text)
                .block(
                    Block::default()
                        .title(" Multi-Worker Download Queue ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(progress_paragraph, popup_rect);
        }
        AppState::Finished {
            completed,
            failed,
            ref message,
        } => {
            let popup_rect = centered_rect(50, 20, size);
            f.render_widget(Clear, popup_rect);

            let finished_text = format!(
                "{}\n\nCompleted successfully: {}\nFailed/Incomplete: {}\n\nPress Enter or Esc to return to menu",
                message, completed, failed
            );

            let finished_paragraph = Paragraph::new(finished_text)
                .block(
                    Block::default()
                        .title(" Task Finished ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(finished_paragraph, popup_rect);
        }
    }
}

fn draw_progress_bar(pct: u16, width: u16) -> String {
    let filled = ((pct as f32 / 100.0) * width as f32).round() as usize;
    let filled = std::cmp::min(filled, width as usize);
    let empty = (width as usize).saturating_sub(filled);
    format!(
        "[{}{}] {}%",
        "█".repeat(filled),
        "░".repeat(empty),
        pct
    )
}

fn get_time_str() -> String {
    unsafe {
        let mut raw_time: libc::time_t = 0;
        libc::time(&mut raw_time);
        let tm_ptr = libc::localtime(&raw_time);
        if !tm_ptr.is_null() {
            let tm = *tm_ptr;
            format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
        } else {
            "00:00:00".to_string()
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
