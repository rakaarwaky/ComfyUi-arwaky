use std::path::{Component, Path, PathBuf};

/// Normalize a path by resolving `.` and `..` components.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            Component::RootDir => {
                result.push(Component::RootDir);
            }
            c => result.push(c),
        }
    }
    result
}

/// Resolve a relative path against a base, then normalize.
#[allow(dead_code)]
pub fn resolve_path(base: &Path, relative: &Path) -> PathBuf {
    normalize_path(&base.join(relative))
}

// ── Path resolution helpers ──

/// Resolve python path from config, with fallback to default install dir.
pub fn resolve_python_path(
    python_path: Option<&str>,
    default_install_dir: Option<&Path>,
) -> PathBuf {
    if let Some(ref path) = python_path {
        return PathBuf::from(path);
    }
    let current_dir = std::env::current_dir().unwrap_or_default();
    let p = current_dir.join("venv/bin/python");
    if p.exists() {
        return p;
    }
    if let Some(install_dir) = default_install_dir {
        let test = install_dir.join("venv/bin/python");
        if test.exists() {
            return test;
        }
    }
    p
}

/// Resolve ComfyUI directory from config, with fallback to default install dir.
pub fn resolve_comfyui_dir(
    comfyui_dir: Option<&str>,
    default_install_dir: Option<&Path>,
) -> PathBuf {
    if let Some(ref path) = comfyui_dir {
        return PathBuf::from(path);
    }
    let current_dir = std::env::current_dir().unwrap_or_default();
    let p = current_dir.join("ComfyUI");
    if p.exists() {
        return p;
    }
    if let Some(install_dir) = default_install_dir {
        let test = install_dir.join("ComfyUI");
        if test.exists() {
            return test;
        }
    }
    p
}

/// Resolve extra_model_paths.yaml path, with fallback to default install dir.
pub fn resolve_extra_model_paths(
    extra_model_paths: Option<&str>,
    default_install_dir: Option<&Path>,
) -> PathBuf {
    if let Some(ref path) = extra_model_paths {
        return PathBuf::from(path);
    }
    let current_dir = std::env::current_dir().unwrap_or_default();
    let p = current_dir.join("extra_model_paths.yaml");
    if p.exists() {
        return p;
    }
    if let Some(install_dir) = default_install_dir {
        let test = install_dir.join("extra_model_paths.yaml");
        if test.exists() {
            return test;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_empty() {
        assert_eq!(normalize_path(Path::new("")), PathBuf::new());
    }

    #[test]
    fn test_normalize_path_normal() {
        assert_eq!(normalize_path(Path::new("/a/b/c")), PathBuf::from("/a/b/c"));
    }

    #[test]
    fn test_normalize_path_dot() {
        assert_eq!(normalize_path(Path::new("/a/./b")), PathBuf::from("/a/b"));
    }

    #[test]
    fn test_normalize_path_double_dot() {
        assert_eq!(
            normalize_path(Path::new("/a/b/../c")),
            PathBuf::from("/a/c")
        );
    }

    #[test]
    fn test_normalize_path_chain() {
        assert_eq!(
            normalize_path(Path::new("/a/b/c/../../d/./e")),
            PathBuf::from("/a/d/e")
        );
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        assert_eq!(normalize_path(Path::new("/a/b/")), PathBuf::from("/a/b"));
    }

    #[test]
    fn test_resolve_path_normal() {
        let base = Path::new("/a/b");
        let relative = Path::new("c/d");
        assert_eq!(resolve_path(base, relative), PathBuf::from("/a/b/c/d"));
    }

    #[test]
    fn test_resolve_path_dot() {
        let base = Path::new("/a");
        let relative = Path::new("./b");
        assert_eq!(resolve_path(base, relative), PathBuf::from("/a/b"));
    }

    #[test]
    fn test_resolve_path_double_dot() {
        let base = Path::new("/a/b");
        let relative = Path::new("../c");
        assert_eq!(resolve_path(base, relative), PathBuf::from("/a/c"));
    }
}
