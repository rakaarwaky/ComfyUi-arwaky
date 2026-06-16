// PURPOSE: Binary entry — TUI mode using orchestrator via aggregate.
// Launches Ratatui terminal UI. All CLI args redirected to CLI binary.

use downloader::root_downloader_container::build_orchestrator;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;
use downloader_shared::taxonomy_model_vo::Model;

const MODELS_JSON: &str = include_str!("config/models.json");

fn get_models() -> Vec<Model> {
    serde_json::from_str(MODELS_JSON).expect("Failed to parse models.json")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If CLI args are passed, redirect to CLI binary usage message
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        eprintln!(
            "TUI mode does not accept arguments. Use the CLI binary for --status / --recommend."
        );
        std::process::exit(1);
    }

    let orch = build_orchestrator();
    let _config = orch.get_config();
    let _models = get_models();

    // Delegate to TUI surface
    downloader_tui::run()
}
