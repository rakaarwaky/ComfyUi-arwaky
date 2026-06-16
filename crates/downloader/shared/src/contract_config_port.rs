use crate::taxonomy_config_vo::Config;

/// Port for loading configuration from any source.
pub trait ConfigPort: Send + Sync {
    fn load(&self) -> Config;
}
