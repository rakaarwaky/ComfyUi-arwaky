// PURPOSE: Log message value object — variants for stdout/stderr/launcher log sources.

use crate::taxonomy_log_level_vo::LogLevel;

/// Log message variants from stdout/stderr/launcher sources.
/// Carries structured metadata for JSON output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogMessage {
    pub level: LogLevel,
    pub source: LogSource,
    pub msg: String,
    pub trace_id: Option<String>,
    pub ts: String,
}

/// Source of the log message — stdout/stderr from backend, or launcher system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    Stdout,
    Stderr,
    Launcher,
}

impl LogMessage {
    pub fn stdout(line: String) -> Self {
        Self {
            level: LogLevel::Info,
            source: LogSource::Stdout,
            msg: line,
            trace_id: None,
            ts: Self::now(),
        }
    }

    pub fn stderr(line: String) -> Self {
        Self {
            level: LogLevel::Error,
            source: LogSource::Stderr,
            msg: line,
            trace_id: None,
            ts: Self::now(),
        }
    }

    pub fn launcher(message: String) -> Self {
        Self {
            level: LogLevel::Info,
            source: LogSource::Launcher,
            msg: message,
            trace_id: None,
            ts: Self::now(),
        }
    }

    pub fn launcher_with_level(level: LogLevel, message: String) -> Self {
        Self {
            level,
            source: LogSource::Launcher,
            msg: message,
            trace_id: None,
            ts: Self::now(),
        }
    }

    pub fn with_trace_id(mut self, id: String) -> Self {
        self.trace_id = Some(id);
        self
    }

    fn now() -> String {
        let duration = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        format!("{}.{:03}", secs, millis)
    }
}

impl std::fmt::Display for LogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string(self) {
            Ok(json) => write!(f, "{}", json),
            Err(_) => write!(
                f,
                "[{}] [{}] {}",
                self.level, self.source, self.msg
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_stdout() {
        let msg = LogMessage::stdout("hi".into());
        let json = msg.to_string();
        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"source\":\"stdout\""));
        assert!(json.contains("\"msg\":\"hi\""));
    }

    #[test]
    fn display_stderr() {
        let msg = LogMessage::stderr("err".into());
        assert!(msg.to_string().contains("\"level\":\"ERROR\""));
    }

    #[test]
    fn display_launcher() {
        let msg = LogMessage::launcher("start".into());
        assert!(msg.to_string().contains("\"source\":\"launcher\""));
    }

    #[test]
    fn debug_stdout() {
        let msg = LogMessage::stdout("x".into());
        assert!(format!("{:?}", msg).contains("stdout"));
    }

    #[test]
    fn with_trace_id() {
        let msg = LogMessage::stdout("x".into()).with_trace_id("abc-123".into());
        let json = msg.to_string();
        assert!(json.contains("\"trace_id\":\"abc-123\""));
    }

    #[test]
    fn launcher_with_level() {
        let msg = LogMessage::launcher_with_level(LogLevel::Warn, "warning".into());
        assert_eq!(msg.level, LogLevel::Warn);
        assert!(msg.to_string().contains("\"level\":\"WARN\""));
    }

    #[test]
    fn serde_roundtrip() {
        let msg = LogMessage::stdout("test".into()).with_trace_id("t1".into());
        let json = serde_json::to_string(&msg).unwrap();
        let back: LogMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, LogSource::Stdout);
        assert_eq!(back.trace_id, Some("t1".into()));
    }
}
