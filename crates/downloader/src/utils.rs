use std::fs;
use std::path::Path;

pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "Unknown".to_string();
    }
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::LazyLock;
use std::path::PathBuf;

pub struct SizeCache {
    pub sizes: HashMap<String, u64>,
}

impl SizeCache {
    pub fn cache_path() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|home| {
            PathBuf::from(home).join(".cache/comfyui-downloader/size_cache.json")
        })
    }

    pub fn load() -> Self {
        if let Some(path) = Self::cache_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(sizes) = serde_json::from_str(&content) {
                        return SizeCache { sizes };
                    }
                }
            }
        }
        SizeCache {
            sizes: HashMap::new(),
        }
    }

    pub fn save(&self) {
        if let Some(path) = Self::cache_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(&self.sizes) {
                let _ = fs::write(path, content);
            }
        }
    }
}

pub static SIZE_CACHE: LazyLock<RwLock<SizeCache>> = LazyLock::new(|| {
    RwLock::new(SizeCache::load())
});

pub fn file_exists_valid(path: &Path, expected_size: u64, url: Option<&str>) -> bool {
    if !path.exists() {
        return false;
    }
    if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            let actual_size = metadata.len();
            let is_valid = |expected: u64| -> bool {
                if expected <= 1_000_000 {
                    actual_size >= 1000
                } else {
                    let min_allowed = (expected as f64 * 0.95) as u64;
                    actual_size >= min_allowed
                }
            };

            if let Some(url_str) = url {
                if let Ok(cache) = SIZE_CACHE.read() {
                    if let Some(&cached_size) = cache.sizes.get(url_str) {
                        if is_valid(cached_size) {
                            return true;
                        }
                    }
                }
            }

            if expected_size == 0 {
                return actual_size >= 1000;
            }

            if is_valid(expected_size) {
                return true;
            }
        }
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            return entries.count() > 5;
        }
    }
    false
}

pub fn get_available_space(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // Find the first ancestor that actually exists
    let mut check_path = path.to_path_buf();
    while !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            break;
        }
    }

    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let c_path = CString::new(check_path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if res == 0 {
        let stats = unsafe { stats.assume_init() };
        Ok(stats.f_bavail as u64 * stats.f_frsize as u64)
    } else {
        Err(std::io::Error::last_os_error())
    }
}
