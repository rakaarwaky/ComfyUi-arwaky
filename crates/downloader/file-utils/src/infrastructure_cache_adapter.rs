// PURPOSE: downloader-file-utils — infrastructure: SizeCache adapter (persistent url→size mapping)

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::RwLock;

use downloader_shared::contract_cache_port::CachePort;

pub struct SizeCache {
    pub sizes: HashMap<String, u64>,
}

impl SizeCache {
    pub fn cache_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".cache/comfyui-downloader/size_cache.json"))
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

impl CachePort for SizeCache {
    fn get_size(&self, url: &str) -> Option<u64> {
        self.sizes.get(url).copied()
    }

    fn set_size(&self, _url: &str, _size: u64) {
        // Interior mutation via RwLock happens at call site (SIZE_CACHE.write())
    }

    fn save(&self) {
        self.save();
    }
}

pub static SIZE_CACHE: LazyLock<RwLock<SizeCache>> =
    LazyLock::new(|| RwLock::new(SizeCache::load()));
