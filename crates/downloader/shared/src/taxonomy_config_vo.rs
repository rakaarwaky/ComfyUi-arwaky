use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub models_dir: String,
    pub hf_token: Option<String>,
    #[serde(default)]
    pub paths: HashMap<String, String>,
}

impl Config {
    pub fn resolve_category_dir(&self, category: &str) -> PathBuf {
        let base = PathBuf::from(&self.models_dir);
        let resolved = if let Some(sub_path) = self.paths.get(category) {
            let sub_path_trimmed = sub_path.trim_matches('/');
            base.join(sub_path_trimmed)
        } else {
            base.join(category)
        };
        // Prevent path traversal: resolved path must stay under models_dir
        if let (Ok(canon_base), Ok(canon_resolved)) =
            (base.canonicalize(), resolved.canonicalize())
        {
            if !canon_resolved.starts_with(&canon_base) {
                // Fallback: ignore paths entry, use category name directly
                return base.join(category);
            }
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config() -> Config {
        let mut paths = HashMap::new();
        paths.insert("checkpoints".to_string(), "checkpoints/".to_string());
        paths.insert("loras".to_string(), "loras/".to_string());
        Config {
            models_dir: "/tmp".to_string(),
            hf_token: None,
            paths,
        }
    }

    #[test]
    fn resolve_category_dir_with_paths_entry() {
        let mut paths = HashMap::new();
        paths.insert("ckpt".to_string(), "checkpoints".to_string());
        let cfg = Config {
            models_dir: "/tmp".to_string(),
            hf_token: None,
            paths,
        };
        let result = cfg.resolve_category_dir("ckpt");
        assert_eq!(result, PathBuf::from("/tmp/checkpoints"));
    }

    #[test]
    fn resolve_category_dir_without_paths_entry() {
        let cfg = test_config();
        let result = cfg.resolve_category_dir("vae");
        assert_eq!(result, PathBuf::from("/tmp/vae"));
    }

    #[test]
    fn resolve_category_dir_strips_trailing_slash() {
        let mut paths = HashMap::new();
        paths.insert("upscale".to_string(), "upscale_models/".to_string());
        let cfg = Config {
            models_dir: "/tmp".to_string(),
            hf_token: None,
            paths,
        };
        let result = cfg.resolve_category_dir("upscale");
        assert_eq!(result, PathBuf::from("/tmp/upscale_models"));
    }

    #[test]
    fn resolve_category_dir_empty_category() {
        let cfg = test_config();
        let result = cfg.resolve_category_dir("");
        assert_eq!(result, PathBuf::from("/tmp/"));
    }

    #[test]
    fn resolve_category_dir_different_models_dir() {
        let cfg = Config {
            models_dir: "/tmp/models".to_string(),
            hf_token: None,
            paths: HashMap::new(),
        };
        let result = cfg.resolve_category_dir("clip");
        assert_eq!(result, PathBuf::from("/tmp/models/clip"));
    }
}
