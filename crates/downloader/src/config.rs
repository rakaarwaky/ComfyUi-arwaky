use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
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
        if let Some(sub_path) = self.paths.get(category) {
            let sub_path_trimmed = sub_path.trim_matches('/');
            base.join(sub_path_trimmed)
        } else {
            base.join(category)
        }
    }
}

pub fn load_config() -> Config {
    let default_cfg_str = include_str!("../config.default.yaml");

    // 1. Try local config.yaml
    if let Ok(content) = fs::read_to_string("config.yaml") {
        if let Ok(cfg) = serde_yaml::from_str(&content) {
            return cfg;
        }
    }
    // 2. Try ~/.config/comfyui-downloader/config.yaml
    if let Ok(home) = std::env::var("HOME") {
        let config_dir = PathBuf::from(&home).join(".config/comfyui-downloader");
        let path = config_dir.join("config.yaml");
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_yaml::from_str(&content) {
                    return cfg;
                }
            }
        } else {
            // Write default config to user config dir
            if let Err(e) = fs::create_dir_all(&config_dir) {
                eprintln!("Warning: Failed to create config directory: {:?}", e);
            } else {
                if let Err(e) = fs::write(&path, default_cfg_str) {
                    eprintln!("Warning: Failed to write default config: {:?}", e);
                }
            }
        }

        // 3. Fallback to launcher's config: ~/.config/comfyui-desktop/config.yaml
        let launcher_path = PathBuf::from(&home).join(".config/comfyui-desktop/config.yaml");
        if let Ok(content) = fs::read_to_string(launcher_path) {
            #[derive(Deserialize)]
            struct LauncherConfig {
                output_dir: Option<String>,
            }
            if let Ok(lcfg) = serde_yaml::from_str::<LauncherConfig>(&content) {
                if let Some(ref out_dir) = lcfg.output_dir {
                    let out_path = PathBuf::from(out_dir);
                    if let Some(parent) = out_path.parent() {
                        let inferred = parent.join("Models");
                        if let Ok(mut parsed_default) = serde_yaml::from_str::<Config>(default_cfg_str) {
                            parsed_default.models_dir = inferred.to_string_lossy().to_string();
                            return parsed_default;
                        }
                        return Config {
                            models_dir: inferred.to_string_lossy().to_string(),
                            hf_token: None,
                            paths: HashMap::new(),
                        };
                    }
                }
            }
        }
    }

    // 4. Default fallback
    if let Ok(cfg) = serde_yaml::from_str::<Config>(default_cfg_str) {
        return cfg;
    }

    let default_models_dir = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join("SharedData/Models").to_string_lossy().to_string()
    } else {
        "./Models".to_string()
    };
    Config {
        models_dir: default_models_dir,
        hf_token: None,
        paths: HashMap::new(),
    }
}
