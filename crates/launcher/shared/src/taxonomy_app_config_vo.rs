// PURPOSE: AppConfig value object for launcher.

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
pub struct AppConfig {
    pub python_path: Option<String>,
    pub comfyui_dir: Option<String>,
    pub extra_model_paths: Option<String>,
    pub output_dir: Option<String>,
    pub input_dir: Option<String>,
    pub user_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_format() {
        let c = AppConfig::default();
        let s = format!("{:?}", c);
        assert!(s.contains("python_path") || s.contains("AppConfig"));
    }

    #[test]
    fn default_values() {
        let c = AppConfig::default();
        assert!(c.python_path.is_none());
        assert!(c.comfyui_dir.is_none());
        assert!(c.extra_model_paths.is_none());
        assert!(c.output_dir.is_none());
    }

    #[test]
    fn default_serde() {
        let c = AppConfig::default();
        let j = serde_json::to_string(&c).unwrap();
        let d: AppConfig = serde_json::from_str(&j).unwrap();
        assert!(d.python_path.is_none());
    }
}
