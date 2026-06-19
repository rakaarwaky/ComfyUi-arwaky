// PURPOSE: Domain error types for config loading.

use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    NotFound(String),
    ParseFailed(String),
    CreateFailed(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "Config not found at: {path}"),
            Self::ParseFailed(msg) => write!(f, "Config parse failed: {msg}"),
            Self::CreateFailed(msg) => write!(f, "Config create failed: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_not_found() {
        assert_eq!(
            ConfigError::NotFound("cfg".into()).to_string(),
            "Config not found at: cfg"
        );
    }
    #[test]
    fn display_parse_failed() {
        assert_eq!(
            ConfigError::ParseFailed("bad".into()).to_string(),
            "Config parse failed: bad"
        );
    }
    #[test]
    fn display_create_failed() {
        assert_eq!(
            ConfigError::CreateFailed("perm".into()).to_string(),
            "Config create failed: perm"
        );
    }
}
