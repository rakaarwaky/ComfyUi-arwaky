// PURPOSE: downloader-dl — agent: orchestrator. ONLY imports contract traits.
// No direct imports to capabilities or infrastructure. Delegates via port + protocol.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use downloader_shared::contract_cache_port::CachePort;
use downloader_shared::contract_config_port::ConfigPort;
use downloader_shared::contract_download_protocol::DownloadProtocol;
use downloader_shared::contract_downloader_aggregate::DownloaderAggregate;
use downloader_shared::contract_file_port::FileValidationPort;
use downloader_shared::contract_file_protocol::FileValidationProtocol;
use downloader_shared::taxonomy_config_vo::Config;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

pub struct DownloaderOrchestrator {
    pub config_port: Arc<dyn ConfigPort>,
    pub file_port: Arc<dyn FileValidationPort>,
    pub file_protocol: Arc<dyn FileValidationProtocol>,
    pub cache_port: Arc<dyn CachePort>,
    pub download_protocol: Arc<dyn DownloadProtocol>,
}

impl DownloaderAggregate for DownloaderOrchestrator {
    fn get_models(&self) -> Vec<Model> {
        Vec::new()
    }

    fn get_config(&self) -> Config {
        self.config_port.load()
    }

    fn file_exists_valid(&self, path: &Path, expected: u64, url: Option<&str>) -> bool {
        self.file_port.file_exists_valid(path, expected, url)
    }

    fn get_cached_size(&self, url: &str) -> Option<u64> {
        self.cache_port.get_size(url)
    }

    fn set_cached_size(&self, url: &str, size: u64) {
        self.cache_port.set_size(url, size);
    }

    fn save_cache(&self) {
        self.cache_port.save();
    }

    fn get_available_space(&self, path: &Path) -> u64 {
        self.file_port.get_available_space(path).unwrap_or(u64::MAX)
    }

    fn start_download_coordinator(
        &self,
        selected: Vec<(usize, Model)>,
        config: Config,
        cancel: Arc<AtomicBool>,
    ) -> Receiver<DownloadEvent> {
        let (tx, rx) = channel();
        let sorted = sort_by_size(selected, &*self.cache_port);
        let config = config.clone();
        let dl = self.download_protocol.clone();
        let file_p = self.file_port.clone();
        let cache = self.cache_port.clone();
        std::thread::spawn(move || {
            coordinator_thread(sorted, config, cancel, tx, dl, file_p, cache)
        });
        rx
    }

    fn refresh_metadata(
        &self,
        models: Vec<(usize, Model)>,
        config: Config,
    ) -> Receiver<DownloadEvent> {
        let (tx, rx) = channel();
        let config = config.clone();
        std::thread::spawn(move || {
            let token = std::env::var("HF_TOKEN").ok().or(config.hf_token.clone());
            let agent = ureq::Agent::new_with_config(
                ureq::config::Config::builder()
                    .timeout_connect(Some(Duration::from_secs(10)))
                    .timeout_recv_body(Some(Duration::from_secs(120)))
                    .timeout_global(Some(Duration::from_secs(60)))
                    .build(),
            );
            let n_workers = models.len().clamp(1, 10);
            let queue = Arc::new(std::sync::Mutex::new(models));
            let tx = Arc::new(tx);
            let token = Arc::new(token);
            let mut handles = Vec::new();
            for _ in 0..n_workers {
                let q = Arc::clone(&queue);
                let t = Arc::clone(&tx);
                let tk = Arc::clone(&token);
                let ag = agent.clone();
                handles.push(std::thread::spawn(move || loop {
                    let item = {
                        let mut l = q.lock().expect("mutex poisoned");
                        l.pop()
                    };
                    let Some((_idx, m)) = item else { break };
                    let mut req = ag.head(&m.url).header("User-Agent", "Mozilla/5.0");
                    if let Some(ref t) = *tk {
                        req = req.header("Authorization", &format!("Bearer {t}"));
                    }
                    match req.call() {
                        Ok(res) => {
                            let s = res.status().as_u16();
                            if s == 200 || s == 206 {
                                let len: u64 = res
                                    .headers()
                                    .get("Content-Length")
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(0);
                                if len > 0 {
                                    // Send size update to TUI before saving to cache
                                    let _ = t.send(DownloadEvent::RefreshUpdate {
                                        idx: _idx,
                                        size: len,
                                    });
                                }
                            }
                        }
                        Err(ureq::Error::StatusCode(404)) => {
                            let _ = t.send(DownloadEvent::RefreshUpdate { idx: _idx, size: 0 });
                        }
                        _ => {}
                    }
                }));
            }
            for h in handles {
                let _ = h.join();
            }
            let _ = tx.send(DownloadEvent::RefreshFinished {
                valid: 0,
                invalid: 0,
                unknown: 0,
            });
        });
        rx
    }
}

fn sort_by_size(selected: Vec<(usize, Model)>, cache: &dyn CachePort) -> Vec<(usize, Model)> {
    let mut s = selected;
    s.sort_by(|(_, a), (_, b)| {
        let la = a.size_bytes.max(cache.get_size(&a.url).unwrap_or(0));
        let lb = b.size_bytes.max(cache.get_size(&b.url).unwrap_or(0));
        let sa = if la == 0 { u64::MAX } else { la };
        let sb = if lb == 0 { u64::MAX } else { lb };
        sa.cmp(&sb).then_with(|| a.filename.cmp(&b.filename))
    });
    s
}

fn coordinator_thread(
    selected: Vec<(usize, Model)>,
    config: Config,
    cancel: Arc<AtomicBool>,
    tx: Sender<DownloadEvent>,
    dl: Arc<dyn DownloadProtocol>,
    _file: Arc<dyn FileValidationPort>,
    _cache: Arc<dyn CachePort>,
) {
    // Pass config token to env for download_file (which reads HF_TOKEN env var)
    if std::env::var("HF_TOKEN").is_err() {
        if let Some(ref token) = config.hf_token {
            std::env::set_var("HF_TOKEN", token);
        }
    }
    let queue = Arc::new(std::sync::Mutex::new(selected));
    let ok_c = Arc::new(std::sync::Mutex::new(0usize));
    let fail_c = Arc::new(std::sync::Mutex::new(0usize));

    let mut handles = Vec::new();
    for wid in 0..1 {
        let (q, c, cn, t, oc, fc) = (
            queue.clone(),
            config.clone(),
            cancel.clone(),
            tx.clone(),
            ok_c.clone(),
            fail_c.clone(),
        );
        let dl = dl.clone();
        handles.push(std::thread::spawn(move || loop {
            let item = {
                let mut l = q.lock().expect("mutex poisoned");
                l.pop()
            };
            let Some((_idx, model)) = item else { break };
            if cn.load(Ordering::Acquire) {
                break;
            }

            // Send Start event so TUI shows progress
            let _ = t.send(DownloadEvent::Start {
                worker_id: wid,
                filename: model.filename.clone(),
            });

            let mut ok = false;
            let mut err = None;
            for attempt in 1..=3 {
                if cn.load(Ordering::Acquire) {
                    break;
                }
                // Use download_one_model for full progress + SHA256 + parallel support
                match dl.download_one_model(wid, &model, &c, &cn, &t) {
                    Ok(_) => {
                        ok = true;
                        break;
                    }
                    Err(ref e) => {
                        // 4xx errors won't resolve by retrying — fail fast
                        if e.contains("401")
                            || e.contains("403")
                            || e.contains("404")
                            || e.contains("Not Found")
                        {
                            err = Some(format!("{e} (skipped retry)"));
                            break;
                        }
                        err = Some(e.clone());
                        if cn.load(Ordering::Acquire) {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(attempt * 2));
                    }
                }
            }
            let _ = t.send(DownloadEvent::ModelFinished {
                worker_id: wid,
                filename: model.filename.clone(),
                success: ok,
                error_msg: err,
            });
            if ok {
                *oc.lock().expect("mutex poisoned") += 1;
            } else {
                *fc.lock().expect("mutex poisoned") += 1;
            }
        }));
    }
    // Wait for all workers to finish before signalling completion
    for h in handles {
        let _ = h.join();
    }
    let _ = tx.send(DownloadEvent::AllComplete {
        completed: *ok_c.lock().expect("mutex poisoned"),
        failed: *fail_c.lock().expect("mutex poisoned"),
    });
}
