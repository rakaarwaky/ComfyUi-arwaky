// PURPOSE: Value object for backend install directory path.
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InstallDir(pub PathBuf);

impl InstallDir {
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

impl From<PathBuf> for InstallDir {
    fn from(p: PathBuf) -> Self {
        Self(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_pathbuf() {
        let dir = InstallDir::from(PathBuf::from("/test/path"));
        assert_eq!(dir.0, PathBuf::from("/test/path"));
    }

    #[test]
    fn as_path_returns_inner() {
        let dir = InstallDir(PathBuf::from("/test"));
        assert_eq!(dir.as_path(), std::path::Path::new("/test"));
    }

    #[test]
    fn debug_format() {
        let dir = InstallDir(PathBuf::from("/x"));
        assert!(format!("{:?}", dir).contains("/x"));
    }
}
