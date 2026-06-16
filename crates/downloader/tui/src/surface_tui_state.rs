// PURPOSE: downloader-tui — surface: app state definitions

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, OnceLock};

use ratatui::widgets::ListState;

use downloader_shared::taxonomy_config_vo::Config;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

pub(super) static SESSION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveDownload {
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed_mb_s: f64,
    pub eta_secs: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum InputMode { Normal, Search }

#[derive(Clone, PartialEq)]
pub enum AppState {
    Menu,
    Settings { active_field: usize, models_dir_input: String, hf_token_input: String },
    DiskSpaceWarning { required: u64, available: u64 },
    Finished { completed: usize, failed: usize, message: String },
}

pub struct App {
    pub models: Vec<Model>,
    pub config: Config,
    pub list_state: ListState,
    pub selected_indices: Vec<usize>,
    pub state: AppState,
    pub rx: Option<Receiver<DownloadEvent>>,
    pub cancel_token: Arc<AtomicBool>,
    pub input_mode: InputMode,
    pub search_query: String,
    pub filtered_cache: Vec<(usize, Model)>,
    pub filtered_cache_dirty: bool,
    pub logs: Vec<String>,
    pub active_tab: usize,
    pub tab_offset: usize,
    pub categories: Vec<String>,
    pub active_downloads: Vec<Option<ActiveDownload>>,
    pub completed_count: usize,
    pub failed_count: usize,
    pub total_to_download: usize,
    pub log_viewer_visible: bool,
    pub log_scroll: usize,
}

impl App {
    pub fn new(models: Vec<Model>, config: Config) -> Self {
        let mut list_state = ListState::default();
        if !models.is_empty() { list_state.select(Some(0)); }
        let mut cats: Vec<String> = models.iter().map(|m| m.category.clone()).collect();
        cats.sort(); cats.dedup();
        let log_path = Self::session_path();
        if let Some(parent) = log_path.parent() { let _ = std::fs::create_dir_all(parent); }
        let _ = SESSION_LOG_PATH.set(log_path);
        let mut app = Self {
            models, config, list_state,
            selected_indices: Vec::new(), state: AppState::Menu, rx: None,
            cancel_token: Arc::new(AtomicBool::new(false)),
            input_mode: InputMode::Normal, search_query: String::new(),
            filtered_cache: Vec::new(), filtered_cache_dirty: true,
            logs: Vec::new(), active_tab: 0, tab_offset: 0, categories: cats,
            active_downloads: vec![None; 2],
            completed_count: 0, failed_count: 0, total_to_download: 0,
            log_viewer_visible: false, log_scroll: 0,
        };
        app.add_log("Downloader initialized successfully.");
        app
    }

    fn session_path() -> PathBuf {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".cache/comfyui-downloader/logs").join(format!("session-{secs}.log"))
    }
}
