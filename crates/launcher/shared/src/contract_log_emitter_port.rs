// PURPOSE: Port for log emission — sending log messages to channel and frontend.

use crate::taxonomy_log_level_vo::LogLevel;

/// Port for emitting log messages to the system.
pub trait LogEmitterPort: Send + Sync {
    /// Send a log message with specified level.
    fn log_with_level(&self, level: LogLevel, message: &str);

    /// Send an info log message.
    fn log_info(&self, message: &str);

    /// Send a warning log message.
    fn log_warn(&self, message: &str);

    /// Send an error log message.
    fn log_error(&self, message: &str);
}
