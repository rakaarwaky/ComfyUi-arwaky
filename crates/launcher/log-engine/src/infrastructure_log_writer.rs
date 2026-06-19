// PURPOSE: Log emitter infrastructure — implements LogEmitterPort with channel sender.

use std::sync::mpsc::SyncSender;

use launcher_shared::contract_log_emitter_port::LogEmitterPort;
use launcher_shared::{LogLevel, LogMessage};

/// Infrastructure adapter: emits log messages via MPSC channel.
pub struct LogEmitter {
    tx: SyncSender<LogMessage>,
}

impl LogEmitter {
    pub fn new(tx: SyncSender<LogMessage>) -> Self {
        Self { tx }
    }
}

impl LogEmitterPort for LogEmitter {
    fn log_with_level(&self, level: LogLevel, message: &str) {
        if self
            .tx
            .try_send(LogMessage::launcher_with_level(level, message.to_string()))
            .is_err()
        {
            eprintln!("[Launcher] {}", message);
        }
    }

    fn log_info(&self, message: &str) {
        self.log_with_level(LogLevel::Info, message);
    }

    fn log_warn(&self, message: &str) {
        self.log_with_level(LogLevel::Warn, message);
    }

    fn log_error(&self, message: &str) {
        self.log_with_level(LogLevel::Error, message);
    }
}
