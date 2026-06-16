// PURPOSE: downloader-cli — CLI: list models & download specific model by name.
// Uses orchestrator via aggregate.

use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use downloader_dl::agent_downloader_orchestrator::DownloaderOrchestrator;
use downloader_dl::capabilities_download_engine::DownloadEngine;
use downloader_file_utils::capabilities_file_checker::FileChecker;
use downloader_file_utils::infrastructure_cache_adapter::SizeCache;
use downloader_file_utils::infrastructure_fs_adapter::FsAdapter;
use downloader_config::ConfigLoader;
use downloader_shared::contract_cache_port::CachePort;
use downloader_shared::contract_config_port::ConfigPort;
use downloader_shared::contract_download_protocol::DownloadProtocol;
use downloader_shared::contract_file_port::FileValidationPort;
use downloader_shared::contract_file_protocol::FileValidationProtocol;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

const MODELS_JSON: &str = include_str!("../../config/models.json");

fn get_models() -> Vec<Model> {
    serde_json::from_str(MODELS_JSON).expect("Failed to parse models.json")
}

fn build_orch() -> DownloaderOrchestrator {
    DownloaderOrchestrator {
        config_port: Arc::new(ConfigLoader) as Arc<dyn ConfigPort>,
        file_port: Arc::new(FsAdapter) as Arc<dyn FileValidationPort>,
        file_protocol: Arc::new(FileChecker) as Arc<dyn FileValidationProtocol>,
        cache_port: Arc::new(SizeCache::load()) as Arc<dyn CachePort>,
        download_protocol: Arc::new(DownloadEngine) as Arc<dyn DownloadProtocol>,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orch = build_orch();
    let config = orch.get_config();
    let models = get_models();
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("list") => cmd_list(&orch, &models, &config),
        Some("download") => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if name.is_empty() {
                eprintln!("Usage: comfyui-downloader download <filename>");
                std::process::exit(1);
            }
            cmd_download(&orch, &models, &config, name)
        }
        Some("recommend") => cmd_recommend(),
        Some(other) => {
            eprintln!("Unknown command: {other}");
            eprintln!("Commands:");
            eprintln!("  list                 — show all models & status");
            eprintln!("  download <filename>  — download a specific model");
            eprintln!("  recommend            — GPU optimization guide");
            std::process::exit(1);
        }
        None => {
            eprintln!("Usage: comfyui-downloader <command>");
            eprintln!("Commands:");
            eprintln!("  list");
            eprintln!("  download <filename>");
            eprintln!("  recommend");
            std::process::exit(1);
        }
    }
}

fn cmd_list(
    orch: &dyn DownloaderAggregate,
    models: &[Model],
    config: &downloader_shared::taxonomy_config_vo::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Model Collection <<<\n");
    for m in models {
        let dest_dir = config.resolve_category_dir(&m.category);
        let dest_path = dest_dir.join(&m.filename);
        let exists = orch.file_exists_valid(&dest_path, m.size_bytes, Some(&m.url));
        let size = if m.size_bytes > 0 {
            m.size_bytes
        } else {
            orch.get_cached_size(&m.url).unwrap_or(0)
        };
        writeln!(
            std::io::stdout(),
            "  {:<50} {:>12}  {}",
            format!("{}/{}", m.category, m.filename),
            downloader_shared::taxonomy_size_vo::format_size(size),
            if exists { "✓" } else { "✗" },
        )?;
    }
    Ok(())
}

fn cmd_download(
    orch: &dyn DownloaderAggregate,
    models: &[Model],
    config: &downloader_shared::taxonomy_config_vo::Config,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find model by filename (case-insensitive, partial match)
    let matches: Vec<(usize, Model)> = models
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.filename.to_lowercase().contains(&name.to_lowercase())
                || m.category.to_lowercase().contains(&name.to_lowercase())
        })
        .map(|(i, m)| (i, m.clone()))
        .collect();

    if matches.is_empty() {
        eprintln!("No model found matching \"{name}\".");
        std::process::exit(1);
    }

    if matches.len() > 1 {
        eprintln!("Multiple models match \"{name}\":");
        for (_, m) in &matches {
            eprintln!("  {}/{}", m.category, m.filename);
        }
        std::process::exit(1);
    }

    let (idx, model) = &matches[0];
    let dest_dir = config.resolve_category_dir(&model.category);
    let dest_path = dest_dir.join(&model.filename);

    // Check if already exists
    if orch.file_exists_valid(&dest_path, model.size_bytes, Some(&model.url)) {
        println!("✓ Already downloaded: {}/{}", model.category, model.filename);
        return Ok(());
    }

    println!("Downloading: {}/{}", model.category, model.filename);
    let cancel = Arc::new(AtomicBool::new(false));
    let rx = orch.start_download_coordinator(
        vec![(*idx, model.clone())],
        config.clone(),
        cancel.clone(),
    );

    // Poll events synchronously
    let mut completed = false;
    let mut failed = false;
    while let Ok(event) = rx.recv() {
        match event {
            DownloadEvent::Start { filename, .. } => {
                println!("  Starting: {filename}");
            }
            DownloadEvent::Progress {
                filename,
                downloaded,
                total,
                speed_mb_s,
                ..
            } => {
                let pct = if total > 0 {
                    (downloaded as f64 / total as f64 * 100.0) as u8
                } else {
                    0
                };
                print!("\r  {}: {}% ({:.1} MB/s)     ", filename, pct, speed_mb_s);
                std::io::stdout().flush()?;
            }
            DownloadEvent::ModelFinished {
                filename,
                success,
                error_msg,
                ..
            } => {
                if success {
                    println!("\r  ✓ Downloaded: {filename}        ");
                    completed = true;
                } else {
                    println!("\r  ✗ Failed: {filename} — {}", error_msg.unwrap_or_default());
                    failed = true;
                }
            }
            DownloadEvent::AllComplete { .. } => break,
            _ => {}
        }
    }

    if completed {
        println!("Done.");
    } else if failed {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_recommend() -> Result<(), Box<dyn std::error::Error>> {
    println!("RX6800XT 16GB VRAM — Optimal Settings Guide\n");
    println!("FLUX Dev (Text-to-Image) GGUF: flux1-dev-Q5_K_S.gguf (~8.3 GB)");
    println!("FLUX Fill (Inpaint/Outpaint) GGUF: flux1-fill-dev-Q4_K_S.gguf (~12 GB)");
    println!("Tips:");
    println!("  • GGUF quants (Q5_K_S) recommended for FLUX Dev");
    println!("  • FP8 quants for memory efficiency");
    println!("  • Set HSA_OVERRIDE_GFX_VERSION=10.3.0 for ROCm");
    Ok(())
}
