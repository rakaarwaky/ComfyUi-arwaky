// PURPOSE: Binary entry — CLI mode (--status, --recommend) using orchestrator via aggregate.
// No TUI. Pure command-line output.

use std::io::Write;

use downloader::root_downloader_container::build_orchestrator;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;
use downloader_shared::taxonomy_model_vo::Model;

const MODELS_JSON: &str = include_str!("config/models.json");

fn get_models() -> Vec<Model> {
    serde_json::from_str(MODELS_JSON).expect("Failed to parse models.json")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orch = build_orchestrator();
    let config = orch.get_config();
    let models = get_models();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--status") => cmd_status(&orch, &models, &config),
        Some("--recommend") | Some("--rx6800xt") | Some("--amd") => cmd_recommend(),
        Some(other) => {
            eprintln!("Unknown argument: {other}");
            eprintln!("Usage: comfyui-downloader [--status | --recommend | --rx6800xt | --amd]");
            std::process::exit(1);
        }
        None => {
            eprintln!("CLI mode requires an argument.");
            eprintln!("Usage: comfyui-downloader --status");
            std::process::exit(1);
        }
    }
}

fn cmd_status(
    orch: &dyn DownloaderAggregate,
    models: &[Model],
    config: &downloader_shared::taxonomy_config_vo::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Model Collection Status <<<\n");
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
            "{:<55} {:>12} {}",
            format!("{}/{}", m.category, m.filename),
            downloader_shared::taxonomy_size_vo::format_size(size),
            if exists {
                "\x1b[32m✓ READY\x1b[0m"
            } else {
                "\x1b[31m✗ MISSING\x1b[0m"
            },
        )?;
    }
    Ok(())
}

fn cmd_recommend() -> Result<(), Box<dyn std::error::Error>> {
    println!("RX6800XT 16GB VRAM - Optimal Settings Guide\n");
    println!("FLUX Dev (Text-to-Image) GGUF Recommended: flux1-dev-Q5_K_S.gguf (~8.3 GB)");
    println!("FLUX Fill (Inpaint/Outpaint) GGUF Recommended: flux1-fill-dev-Q4_K_S.gguf (~12 GB)");
    Ok(())
}
