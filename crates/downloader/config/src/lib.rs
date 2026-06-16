// PURPOSE: downloader-config — capabilities: load config from multi-source fallback

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use downloader_shared::contract_config_port::ConfigPort;
use downloader_shared::taxonomy_config_vo::Config;

pub struct ConfigLoader;

const DEFAULT_CONFIG: &str = include_str!("../config.default.yaml");

impl ConfigPort for ConfigLoader {
    fn load(&self) -> Config {
        load_config()
    }
}

pub fn load_config() -> Config {
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
                if let Err(e) = fs::write(&path, DEFAULT_CONFIG) {
                    eprintln!("Warning: Failed to write default config: {:?}", e);
                }
            }
        }

        // 3. Fallback to launcher's config: ~/.config/comfyui-desktop/config.yaml
        let launcher_path = PathBuf::from(&home).join(".config/comfyui-desktop/config.yaml");
        if let Ok(content) = fs::read_to_string(launcher_path) {
            #[derive(serde::Deserialize)]
            struct LauncherConfig {
                output_dir: Option<String>,
            }
            if let Ok(lcfg) = serde_yaml::from_str::<LauncherConfig>(&content) {
                if let Some(ref out_dir) = lcfg.output_dir {
                    let out_path = PathBuf::from(out_dir);
                    if let Some(parent) = out_path.parent() {
                        let inferred = parent.join("Models");
                        if let Ok(mut parsed_default) =
                            serde_yaml::from_str::<Config>(DEFAULT_CONFIG)
                        {
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
    if let Ok(cfg) = serde_yaml::from_str::<Config>(DEFAULT_CONFIG) {
        return cfg;
    }

    let default_models_dir = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join("SharedData/Models")
            .to_string_lossy()
            .to_string()
    } else {
        "./Models".to_string()
    };
    Config {
        models_dir: default_models_dir,
        hf_token: None,
        paths: HashMap::new(),
    }
}
