// PURPOSE: Port for loading and ensuring launcher configuration.

use crate::taxonomy_app_config_vo::AppConfig;
use crate::taxonomy_config_error_vo::ConfigError;
use crate::taxonomy_install_dir_vo::InstallDir;

pub trait ConfigPort: Send + Sync {
    fn load(&self) -> Result<AppConfig, ConfigError>;
    fn ensure(&self, app_config_dir: &InstallDir) -> Result<AppConfig, ConfigError>;
}
