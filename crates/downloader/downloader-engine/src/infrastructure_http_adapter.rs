// PURPOSE: downloader-dl — infrastructure: HTTP download implementation
// Implements DownloadPort. Handles chunked parallel & single-threaded HTTP downloads.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use downloader_shared::contract_download_port::DownloadPort;
use downloader_shared::taxonomy_config_vo::Config;
use downloader_shared::taxonomy_download_event_vo::DownloadEvent;
use downloader_shared::taxonomy_model_vo::Model;

pub struct HttpDownloadAdapter;

impl DownloadPort for HttpDownloadAdapter {
    fn download_file(&self, url: &str, dest: &Path) -> Result<(), String> {
        let resp = ureq::get(url)
            .header("User-Agent", "Mozilla/5.0")
            .call()
            .map_err(|e| e.to_string())?;
        let sc = resp.status().as_u16();
        if sc != 200 && sc != 206 {
            return Err(match sc { 401 => "Unauthorized".into(), 403 => "Forbidden".into(), 404 => "Not Found".into(), _ => format!("HTTP {sc}") });
        }
        let mut reader = resp.into_body().into_reader();
        let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ChunkProgress {
    pub total_size: u64,
    #[serde(default)]
    pub n_chunks: usize,
    pub chunk_offsets: Vec<u64>,
}

/// Multi-threaded chunked download (range requests).
#[allow(clippy::too_many_arguments)]
pub fn parallel_download(
    _worker_id: usize, model: &Model, _config: &Config, cancel_token: &Arc<AtomicBool>,
    _tx: &Sender<DownloadEvent>, temp_path: &Path, total_size: u64,
    _agent: &ureq::Agent, token: &Option<String>,
) -> Result<(), String> {
    let n_chunks: usize = if total_size < 50 * 1024 * 1024 { 1 }
        else if total_size < 200 * 1024 * 1024 { 2 }
        else if total_size < 1024 * 1024 * 1024 { 4 }
        else { 8 };

    let progress_path = temp_path.with_extension("progress");
    let chunk_size = total_size / n_chunks as u64;
    let chunks: Vec<(u64, u64)> = (0..n_chunks).map(|i| {
        let start = i as u64 * chunk_size;
        let end = if i == n_chunks - 1 { total_size - 1 } else { (i as u64 + 1) * chunk_size - 1 };
        (start, end)
    }).collect();

    let mut chunk_offsets = vec![0u64; n_chunks];
    // Resume from progress file
    if progress_path.exists() && temp_path.exists() {
        if let Ok(content) = fs::read_to_string(&progress_path) {
            if let Ok(p) = serde_json::from_str::<ChunkProgress>(&content) {
                if p.total_size == total_size && p.chunk_offsets.len() == n_chunks {
                    chunk_offsets = p.chunk_offsets;
                }
            }
        }
    }

    let file = fs::OpenOptions::new().write(true).create(true).truncate(true).open(temp_path)
        .map_err(|e| e.to_string())?;
    file.set_len(total_size).map_err(|e| e.to_string())?;
    let shared_file = Arc::new(file);
    let thread_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let shared_agent = Arc::new(ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .timeout_global(Some(Duration::from_secs(3600))).build(),
    ));

    let mut handles = Vec::new();
    for (i, &(start, end)) in chunks.iter().enumerate() {
        let start_pos = start + chunk_offsets[i];
        if start_pos > end { continue; }
        let f = Arc::clone(&shared_file);
        let a = Arc::clone(&shared_agent);
        let te = Arc::clone(&thread_errors);
        let ct = Arc::clone(cancel_token);
        let u = model.url.clone();
        let tk = token.clone();
        let initial = chunk_offsets[i];
        let atomic = Arc::new(AtomicU64::new(initial));

        handles.push(std::thread::spawn(move || {
            let res = (|| -> Result<(), String> {
                let mut req = a.get(&u).header("User-Agent", "Mozilla/5.0")
                    .header("Range", &format!("bytes={start_pos}-{end}"));
                if let Some(ref t) = tk { req = req.header("Authorization", &format!("Bearer {t}")); }
                let resp = req.call().map_err(|e| e.to_string())?;
                let sc = resp.status().as_u16();
                if sc != 200 && sc != 206 { return Err(format!("HTTP {sc}")); }
                let mut reader = resp.into_body().into_reader();
                let mut buf = vec![0u8; 256 * 1024];
                let mut dl = 0u64;
                loop {
                    if ct.load(Ordering::Acquire) { return Err("Cancelled".into()); }
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            f.write_all_at(&buf[..n], start_pos + dl).map_err(|e| e.to_string())?;
                            dl += n as u64;
                            atomic.store(initial + dl, Ordering::Release);
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
                Ok(())
            })();
            if let Err(e) = res { te.lock().expect("mutex poisoned").push(e); }
        }));
    }

    // Monitor progress
    let start_time = Instant::now();
    let _last_report = Instant::now();
    let _last_progress_instant = Instant::now();

    loop {
        if cancel_token.load(Ordering::Acquire) { return Err("Cancelled".into()); }
        if start_time.elapsed() > Duration::from_secs(3600) { return Err("Timeout".into()); }
        { let e = thread_errors.lock().expect("mutex poisoned"); if !e.is_empty() { return Err(e[0].clone()); } }
        if handles.iter().all(|h| h.is_finished()) { break; }
        // Progress tracking simplified
        std::thread::sleep(Duration::from_millis(50));
    }
    for h in handles { let _ = h.join(); }
    let _ = fs::remove_file(&progress_path);
    Ok(())
}

/// Single-threaded download fallback.
#[allow(clippy::too_many_arguments)]
pub fn single_download(
    worker_id: usize, model: &Model, token: &Option<String>, cancel_token: &Arc<AtomicBool>,
    tx: &Sender<DownloadEvent>, temp_path: &Path, agent: &ureq::Agent, total_size: u64,
) -> Result<(), String> {
    let current_size = fs::metadata(temp_path).map(|m| m.len()).unwrap_or(0);
    let mut req = agent.get(&model.url).header("User-Agent", "Mozilla/5.0");
    if let Some(ref t) = token { req = req.header("Authorization", &format!("Bearer {t}")); }
    if current_size > 0 { req = req.header("Range", &format!("bytes={current_size}-")); }

    let resp = req.call().map_err(|e| e.to_string())?;
    let sc = resp.status().as_u16();
    if sc != 200 && sc != 206 {
        return Err(match sc { 401 => "Unauthorized".into(), 403 => "Forbidden".into(), 404 => "Not Found".into(), _ => format!("HTTP {sc}") });
    }
    let is_partial = sc == 206;
    let response_len: u64 = resp.headers().get("Content-Length")
        .and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let total = if is_partial { current_size + response_len }
        else if response_len > 0 { response_len } else { total_size };

    let file = if is_partial { fs::OpenOptions::new().append(true).open(temp_path) }
        else { fs::OpenOptions::new().write(true).create(true).truncate(true).open(temp_path) };
    let file = file.map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    let mut reader = resp.into_body().into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut downloaded: u64 = if is_partial { current_size } else { 0 };
    let start_time = Instant::now();
    let mut last_report = Instant::now();

    loop {
        if cancel_token.load(Ordering::Acquire) { return Err("Cancelled".into()); }
        if start_time.elapsed() > Duration::from_secs(3600) { return Err("Timeout".into()); }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                writer.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                downloaded += n as u64;
                if last_report.elapsed() >= Duration::from_millis(200) {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (downloaded.saturating_sub(if is_partial { current_size } else { 0 })) as f64 / (1024.0 * 1024.0) / elapsed
                    } else { 0.0 };
                    let eta = if speed > 0.0 { ((total.saturating_sub(downloaded)) as f64 / (1024.0 * 1024.0) / speed) as u64 } else { 0 };
                    let _ = tx.send(DownloadEvent::Progress {
                        worker_id, filename: model.filename.clone(),
                        downloaded, total, speed_mb_s: speed, eta_secs: eta,
                    });
                    last_report = Instant::now();
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}
