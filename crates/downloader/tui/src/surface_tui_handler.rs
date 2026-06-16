// PURPOSE: downloader-tui — surface: TUI-only handler (no CLI argument processing)

use std::time::{Duration, Instant};

use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;

use downloader_config::load_config;
use downloader_file_utils::capabilities_file_checker::file_exists_valid;
use downloader_file_utils::infrastructure_cache_adapter::SIZE_CACHE;

use crate::surface_tui_state::App;
use crate::surface_tui_draw::draw_ui;
use crate::surface_tui_event::handle_event;

use downloader_shared::taxonomy_model_vo::Model;

const MODELS_JSON: &str = include_str!("../../config/models.json");

pub fn get_models() -> Vec<Model> {
    serde_json::from_str(MODELS_JSON).expect("Failed to parse models.json")
}

/// Launch TUI mode only. CLI arguments handled by root_cli_main_entry.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    let models = get_models();

    // Background size checking thread — validates model URLs via HEAD
    {
        let config = config.clone();
        let models = models.clone();
        std::thread::spawn(move || {
            let agent = ureq::Agent::new_with_config(
                ureq::config::Config::builder()
                    .timeout_connect(Some(std::time::Duration::from_secs(10)))
                    .timeout_recv_body(Some(std::time::Duration::from_secs(10)))
                    .timeout_global(Some(std::time::Duration::from_secs(15)))
                    .build(),
            );
            for m in &models {
                // Skip models that already exist on disk with valid size
                let dest_dir = config.resolve_category_dir(&m.category);
                let dest_path = dest_dir.join(&m.filename);
                if dest_path.is_file() && file_exists_valid(&dest_path, m.size_bytes, Some(&m.url)) {
                    continue;
                }
                // HEAD probe to check URL + get size
                let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
                let mut req = agent.head(&m.url).header("User-Agent", "Mozilla/5.0");
                if let Some(ref t) = token {
                    req = req.header("Authorization", &format!("Bearer {t}"));
                }
                match req.call() {
                    Ok(res) => {
                        let status = res.status().as_u16();
                        if status == 200 || status == 206 {
                            if let Some(len) = res.headers().get("Content-Length")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|v| v.parse::<u64>().ok())
                            {
                                if len > 0 {
                                    if let Ok(mut cache) = SIZE_CACHE.write() {
                                        cache.sizes.insert(m.url.clone(), len);
                                    }
                                }
                            }
                        }
                    }
                    Err(ureq::Error::StatusCode(404)) => {
                        // 404 — model URL is invalid; skip silently
                    }
                    Err(_) => {} // transport errors — skip, try again later
                }
            }
            if let Ok(cache) = SIZE_CACHE.write() { cache.save(); }
        });
    }

    // TUI mode
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
        let timeout = tick_rate.checked_sub(last_tick.elapsed()).unwrap_or_else(|| Duration::from_secs(0));
        if poll(timeout)? {
            let crossterm_event = read()?;
            let should_continue = handle_event(&mut app, &mut terminal, crossterm_event)?;
            if !should_continue { break; }
        }
        if last_tick.elapsed() >= tick_rate { last_tick = Instant::now(); }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
