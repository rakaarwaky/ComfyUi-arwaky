use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Model {
    pub category: String,
    pub filename: String,
    pub url: String,
    #[serde(default)]
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub group: String,
    pub notes: String,
}
