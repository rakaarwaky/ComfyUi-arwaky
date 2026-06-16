// PURPOSE: downloader — root DI container. Wires concrete implementations into orchestrator.
// This is the ONLY file that imports concrete capabilities + infrastructure.
// Surfaces import this container, NOT the orchestrator directly.

use std::sync::Arc;

use downloader_dl::agent_downloader_orchestrator::DownloaderOrchestrator;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;

/// Build the orchestrator with all its dependencies wired.
pub fn build_orchestrator() -> DownloaderOrchestrator {
    use downloader_config::ConfigLoader;
    use downloader_dl::capabilities_download_engine::DownloadEngine;

    use downloader_file_utils::capabilities_file_checker::FileChecker;
    use downloader_file_utils::infrastructure_cache_adapter::SizeCache;
    use downloader_file_utils::infrastructure_fs_adapter::FsAdapter;
    use downloader_shared::contract_cache_port::CachePort;
    use downloader_shared::contract_config_port::ConfigPort;
    use downloader_shared::contract_download_protocol::DownloadProtocol;
    use downloader_shared::contract_file_port::FileValidationPort;
    use downloader_shared::contract_file_protocol::FileValidationProtocol;

    // Infrastructure (ports)
    let config_port: Arc<dyn ConfigPort> = Arc::new(ConfigLoader);
    let file_port: Arc<dyn FileValidationPort> = Arc::new(FsAdapter);
    let cache_port: Arc<dyn CachePort> = Arc::new(SizeCache::load()); // load persisted cache

    // Capabilities (protocols)
    let file_protocol: Arc<dyn FileValidationProtocol> = Arc::new(FileChecker);
    let download_protocol: Arc<dyn DownloadProtocol> = Arc::new(DownloadEngine);

    DownloaderOrchestrator {
        config_port,
        file_port,
        file_protocol,
        cache_port,
        download_protocol,
    }
}

/// Convenience — get a configured orchestrator as aggregate trait object.
pub fn get_aggregate() -> Arc<dyn DownloaderAggregate> {
    Arc::new(build_orchestrator())
}
