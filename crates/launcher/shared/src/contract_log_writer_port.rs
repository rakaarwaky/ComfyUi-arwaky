// PURPOSE: Port for log file writing — persistent log storage.

use std::path::Path;

/// Port for writing formatted log lines to persistent storage.
pub trait LogWriterPort: Send + Sync {
    /// Write a formatted log line to storage.
    fn write_log(&self, formatted: &str);

    /// Return the path to the log file.
    fn log_path(&self) -> &Path;
}
