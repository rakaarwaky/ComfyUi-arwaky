use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};

pub mod app;
pub mod draw;
pub mod event;

pub use app::{ActiveDownload, App, AppState, InputMode};
pub use draw::draw_ui;
pub use event::handle_event;

use crate::config::load_config;
use crate::model::get_models;
use crate::utils::{file_exists_valid, format_size};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config();
    let models = get_models();

    // Background size checking thread
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

            let needs_check: Vec<_> = models
                .iter()
                .filter(|m| {
                    let dest_dir = config.resolve_category_dir(&m.category);
                    let dest_path = dest_dir.join(&m.filename);
                    dest_path.is_file()
                        && !file_exists_valid(&dest_path, m.size_bytes, Some(&m.url))
                })
                .collect();

            for m in needs_check {
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
                            }
                        }
                    }
                }
            }
            if let Ok(cache) = crate::utils::SIZE_CACHE.write() {
                cache.save();
            }
        });
    }

    // CLI argument handler
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
                        if exists {
                            "\x1b[32m✓ READY\x1b[0m"
                        } else {
                            "\x1b[31m✗ MISSING\x1b[0m"
                        }
                    );
                }
                return Ok(());
            }
            "--recommend" | "--rx6800xt" | "--amd" => {
                println!("RX6800XT 16GB VRAM - Optimal Settings Guide\n");
                println!(
                    "FLUX Dev (Text-to-Image) GGUF Recommended: flux1-dev-Q5_K_S.gguf (~8.3 GB)"
                );
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

        if poll(timeout)? {
            let crossterm_event = read()?;
            let should_continue = handle_event(&mut app, &mut terminal, crossterm_event)?;
            if !should_continue {
                break;
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
