// PURPOSE: File log writer — writes logs to rotating file for ComfyUI extension to read.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use launcher_shared::contract_log_writer_port::LogWriterPort;

const MAX_LOG_FILE_SIZE: u64 = 5 * 1024 * 1024; // 5MB
const LOG_FILE_NAME: &str = "comfyui-backend.log";

pub struct FileLogWriter {
    writer: Mutex<Option<BufWriter<File>>>,
    path: PathBuf,
    size: std::sync::atomic::AtomicU64,
}

impl FileLogWriter {
    pub fn new(cache_dir: &Path) -> Arc<Self> {
        let path = cache_dir.join(LOG_FILE_NAME);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
            .map(BufWriter::new);

        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        Arc::new(Self {
            writer: Mutex::new(writer),
            path,
            size: std::sync::atomic::AtomicU64::new(size),
        })
    }

    fn rotate(&self) {
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        // Truncate and reopen
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
        {
            *guard = Some(BufWriter::new(file));
            self.size.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl LogWriterPort for FileLogWriter {
    fn write_log(&self, formatted: &str) {
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(ref mut writer) = *guard {
            let line = format!("{}\n", formatted);
            if writer.write_all(line.as_bytes()).is_ok() {
                let current_size = self
                    .size
                    .fetch_add(line.len() as u64, std::sync::atomic::Ordering::Relaxed)
                    + line.len() as u64;

                // Rotate if too large
                if current_size >= MAX_LOG_FILE_SIZE {
                    let _ = writer.flush();
                    drop(guard);
                    self.rotate();
                }
            }
        }
    }

    fn log_path(&self) -> &Path {
        &self.path
    }
}
