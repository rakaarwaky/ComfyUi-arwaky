// PURPOSE: Log message value object — variants for stdout/stderr/launcher log sources.

/// Log message variants from stdout/stderr/launcher sources.
pub enum LogMessage {
    Stdout(String),
    Stderr(String),
    Launcher(String),
}

impl std::fmt::Display for LogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdout(line) => write!(f, "[stdout] {line}"),
            Self::Stderr(line) => write!(f, "[stderr] {line}"),
            Self::Launcher(line) => write!(f, "[Launcher] {line}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_stdout() { assert_eq!(LogMessage::Stdout("hi".into()).to_string(), "[stdout] hi"); }
    #[test]
    fn display_stderr() { assert_eq!(LogMessage::Stderr("err".into()).to_string(), "[stderr] err"); }
    #[test]
    fn display_launcher() { assert_eq!(LogMessage::Launcher("start".into()).to_string(), "[Launcher] start"); }
    #[test]
    fn debug_stdout() { assert!(format!("{:?}", LogMessage::Stdout("x".into())).contains("Stdout")); }
}
