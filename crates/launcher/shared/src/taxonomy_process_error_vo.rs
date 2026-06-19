// PURPOSE: Domain error types for process spawning.

use std::fmt;

#[derive(Debug)]
pub enum ProcessError {
    PythonNotFound(String),
    SpawnFailed(String),
    SmokeTestFailed(String),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PythonNotFound(path) => write!(f, "Python binary not found at: {path}"),
            Self::SpawnFailed(msg) => write!(f, "Process spawn failed: {msg}"),
            Self::SmokeTestFailed(msg) => write!(f, "Smoke test failed: {msg}"),
        }
    }
}

impl std::error::Error for ProcessError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_python_not_found() {
        assert_eq!(
            ProcessError::PythonNotFound("/usr/bin".into()).to_string(),
            "Python binary not found at: /usr/bin"
        );
    }
    #[test]
    fn display_spawn_failed() {
        assert_eq!(
            ProcessError::SpawnFailed("err".into()).to_string(),
            "Process spawn failed: err"
        );
    }
    #[test]
    fn display_smoke_test_failed() {
        assert_eq!(
            ProcessError::SmokeTestFailed("fail".into()).to_string(),
            "Smoke test failed: fail"
        );
    }
}
