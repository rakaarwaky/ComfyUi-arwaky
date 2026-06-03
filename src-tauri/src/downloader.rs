use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tar::Archive;

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
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

fn validate_symlink_target(target: &Path, base_dir: &Path) -> Result<(), String> {
    if target.is_absolute() {
        return Err(format!("Absolute symlink rejected: {}", target.display()));
    }

    for component in target.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "Symlink with path traversal rejected: {}",
                target.display()
            ));
        }
    }

    let resolved = normalize_path(&base_dir.join(target));
    let normalized_base = normalize_path(base_dir);
    if !resolved.starts_with(&normalized_base) {
        return Err(format!(
            "Symlink escapes install directory: {} -> {}",
            base_dir.display(),
            target.display()
        ));
    }

    Ok(())
}

const BACKEND_VERSION: &str = "1.1.0";
const BACKEND_ARCHIVE_NAME: &str = "comfyui-backend-linux-x86_64.tar.gz";

#[derive(Clone, Debug, serde::Serialize)]
pub struct DownloadProgress {
    pub phase: Phase,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum Phase {
    Downloading,
    Verifying,
    Extracting,
    Complete,
}

pub struct BackendInstaller {
    install_dir: PathBuf,
    archive_url: String,
    expected_sha256: Option<String>,
}

impl BackendInstaller {
    pub fn new(install_dir: PathBuf, archive_url: String, expected_sha256: Option<String>) -> Self {
        Self {
            install_dir,
            archive_url,
            expected_sha256,
        }
    }

    pub fn is_installed(&self) -> bool {
        let python_path = self.install_dir.join("venv/bin/python");
        let main_py = self.install_dir.join("ComfyUI/main.py");
        python_path.exists() && main_py.exists()
    }

    pub fn installed_version(&self) -> Option<String> {
        let version_file = self.install_dir.join("version.txt");
        fs::read_to_string(version_file)
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub fn check_disk_space(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let output = std::process::Command::new("df")
                .arg("-B1")
                .arg("--output=avail")
                .arg(&self.install_dir)
                .output()
                .map_err(|e| format!("Failed to check disk space: {}", e))?;
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(bytes_str) = stdout.lines().last() {
                    if let Ok(avail) = bytes_str.trim().parse::<u64>() {
                        let needed = 20u64 * 1024 * 1024 * 1024;
                        if avail < needed {
                            return Err(format!(
                                "Insufficient disk space: {} GB available, need at least 20 GB",
                                avail / (1024 * 1024 * 1024)
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn install<F>(
        &self,
        progress_callback: F,
        cancel_token: Arc<AtomicBool>,
    ) -> Result<(), String>
    where
        F: Fn(DownloadProgress) + Send + Sync,
    {
        if cancel_token.load(Ordering::Acquire) {
            return Err("Download cancelled".to_string());
        }

        fs::create_dir_all(&self.install_dir)
            .map_err(|e| format!("Failed to create install directory: {}", e))?;

        self.check_disk_space()?;

        let tmp_dir = self.install_dir.join(".tmp");
        let archive_path = tmp_dir.join(BACKEND_ARCHIVE_NAME);

        if tmp_dir.exists() {
            fs::remove_dir_all(&tmp_dir)
                .map_err(|e| format!("Failed to clean temp directory: {}", e))?;
        }
        fs::create_dir_all(&tmp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        progress_callback(DownloadProgress {
            phase: Phase::Downloading,
            bytes_downloaded: 0,
            total_bytes: 0,
            message: "Starting download...".to_string(),
        });

        self.download_archive(&archive_path, &progress_callback, &cancel_token)?;

        if let Some(ref expected) = self.expected_sha256 {
            progress_callback(DownloadProgress {
                phase: Phase::Verifying,
                bytes_downloaded: 0,
                total_bytes: 0,
                message: "Verifying checksum...".to_string(),
            });
            self.verify_checksum(&archive_path, expected)?;
        }

        progress_callback(DownloadProgress {
            phase: Phase::Extracting,
            bytes_downloaded: 0,
            total_bytes: 0,
            message: "Extracting backend...".to_string(),
        });

        self.extract_archive(&archive_path, &progress_callback, &cancel_token)?;

        self.finalize_installation()?;

        fs::write(self.install_dir.join("version.txt"), BACKEND_VERSION)
            .map_err(|e| format!("Failed to write version file: {}", e))?;

        let _ = fs::remove_dir_all(&tmp_dir);

        progress_callback(DownloadProgress {
            phase: Phase::Complete,
            bytes_downloaded: 0,
            total_bytes: 0,
            message: "Backend installation complete!".to_string(),
        });

        Ok(())
    }

    fn download_archive<F>(
        &self,
        dest: &Path,
        progress_callback: &F,
        cancel_token: &AtomicBool,
    ) -> Result<(), String>
    where
        F: Fn(DownloadProgress) + Send + Sync,
    {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(std::time::Duration::from_secs(30)))
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build(),
        );

        let response = agent.get(&self.archive_url).call().map_err(|e| match e {
            ureq::Error::StatusCode(code) => {
                format!("Download failed: server returned HTTP {}", code)
            }
            _ => format!("Failed to connect to download server: {}", e),
        })?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(format!("Download failed: server returned HTTP {}", status));
        }

        let total_bytes: u64 = response
            .headers()
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let file =
            fs::File::create(dest).map_err(|e| format!("Failed to create archive file: {}", e))?;
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);

        let mut reader = response.into_body().into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut bytes_downloaded: u64 = 0;
        let mut last_report = std::time::Instant::now();

        loop {
            if cancel_token.load(Ordering::Acquire) {
                return Err("Download cancelled".to_string());
            }

            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Download read error: {}", e))?;
            if n == 0 {
                break;
            }

            writer
                .write_all(&buf[..n])
                .map_err(|e| format!("Download write error: {}", e))?;

            bytes_downloaded += n as u64;

            if last_report.elapsed() >= std::time::Duration::from_millis(500)
                || bytes_downloaded == total_bytes
            {
                progress_callback(DownloadProgress {
                    phase: Phase::Downloading,
                    bytes_downloaded,
                    total_bytes,
                    message: format!(
                        "Downloading... {} / {} MB",
                        bytes_downloaded / (1024 * 1024),
                        total_bytes / (1024 * 1024)
                    ),
                });
                last_report = std::time::Instant::now();
            }
        }

        writer
            .flush()
            .map_err(|e| format!("Failed to flush download: {}", e))?;

        Ok(())
    }

    fn verify_checksum(&self, file_path: &Path, expected: &str) -> Result<(), String> {
        let file = fs::File::open(file_path)
            .map_err(|e| format!("Failed to open archive for verification: {}", e))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();

        let mut buf = [0u8; 256 * 1024];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Checksum read error: {}", e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        let computed = hex::encode(hasher.finalize());
        if computed != expected {
            return Err(format!(
                "Checksum mismatch: expected {}, got {}",
                expected, computed
            ));
        }

        Ok(())
    }

    fn extract_archive<F>(
        &self,
        archive_path: &Path,
        progress_callback: &F,
        cancel_token: &AtomicBool,
    ) -> Result<(), String>
    where
        F: Fn(DownloadProgress) + Send + Sync,
    {
        let file =
            fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let tmp_extract = self.install_dir.join(".tmp_extract");
        if tmp_extract.exists() {
            fs::remove_dir_all(&tmp_extract)
                .map_err(|e| format!("Failed to clean extraction directory: {}", e))?;
        }
        fs::create_dir_all(&tmp_extract)
            .map_err(|e| format!("Failed to create extraction directory: {}", e))?;

        let mut extracted = 0u64;
        let entries = archive
            .entries()
            .map_err(|e| format!("Failed to read archive entries: {}", e))?;

        for entry in entries {
            if cancel_token.load(Ordering::Acquire) {
                return Err("Extraction cancelled".to_string());
            }

            let mut entry = entry.map_err(|e| format!("Failed to read archive entry: {}", e))?;

            let entry_path = entry
                .path()
                .map_err(|e| format!("Invalid archive entry path: {}", e))?
                .into_owned();

            let path_str = entry_path.to_string_lossy();
            if path_str.contains("..") || path_str.starts_with('/') {
                continue;
            }

            let dest = tmp_extract.join(&entry_path);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create directory {}: {}", parent.display(), e)
                })?;
            }

            let entry_type = entry.header().entry_type();
            match entry_type {
                tar::EntryType::Symlink => {
                    let link_target = entry
                        .link_name()
                        .map_err(|e| format!("Failed to read symlink target: {}", e))?
                        .ok_or_else(|| {
                            format!("Symlink entry {} has no target", entry_path.display())
                        })?
                        .into_owned();

                    if let Err(e) = validate_symlink_target(&link_target, &tmp_extract) {
                        eprintln!(
                            "Warning: Skipping unsafe symlink {} -> {}: {}",
                            entry_path.display(),
                            link_target.display(),
                            e
                        );
                        continue;
                    }

                    if dest.exists() || dest.symlink_metadata().is_ok() {
                        let _ = fs::remove_file(&dest);
                    }

                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&link_target, &dest).map_err(|e| {
                        format!(
                            "Failed to create symlink {} -> {}: {}",
                            dest.display(),
                            link_target.display(),
                            e
                        )
                    })?;
                    #[cfg(not(unix))]
                    {
                        let _ = &link_target;
                        eprintln!(
                            "Warning: Skipping symlink on non-Unix: {}",
                            entry_path.display()
                        );
                        continue;
                    }
                }
                tar::EntryType::Directory => {
                    fs::create_dir_all(&dest).map_err(|e| {
                        format!("Failed to create directory {}: {}", dest.display(), e)
                    })?;
                }
                tar::EntryType::Link => {
                    eprintln!("Warning: Skipping hardlink: {}", entry_path.display());
                    continue;
                }
                _ => {
                    entry.unpack(&dest).map_err(|e| {
                        format!("Failed to extract {}: {}", entry_path.display(), e)
                    })?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(mode) = entry.header().mode() {
                            let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(mode));
                        }
                    }
                }
            }

            extracted += 1;
            if extracted % 100 == 0 {
                progress_callback(DownloadProgress {
                    phase: Phase::Extracting,
                    bytes_downloaded: extracted,
                    total_bytes: 0,
                    message: format!("Extracting... {} files", extracted),
                });
            }
        }

        Ok(())
    }

    fn finalize_installation(&self) -> Result<(), String> {
        let tmp_extract = self.install_dir.join(".tmp_extract");

        if tmp_extract.exists() {
            for entry in fs::read_dir(&tmp_extract)
                .map_err(|e| format!("Failed to read extracted directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let src = entry.path();
                let file_name = entry.file_name();
                let dest = self.install_dir.join(&file_name);

                if dest.exists() {
                    if dest.is_dir() {
                        fs::remove_dir_all(&dest).map_err(|e| {
                            format!("Failed to remove old {}: {}", dest.display(), e)
                        })?;
                    } else {
                        fs::remove_file(&dest).map_err(|e| {
                            format!("Failed to remove old {}: {}", dest.display(), e)
                        })?;
                    }
                }

                fs::rename(&src, &dest).map_err(|e| {
                    format!(
                        "Failed to move {} to {}: {}",
                        src.display(),
                        dest.display(),
                        e
                    )
                })?;
            }

            fs::remove_dir_all(&tmp_extract).ok();
        }

        let python_path = self.install_dir.join("venv/bin/python");
        let main_py = self.install_dir.join("ComfyUI/main.py");
        let version_file = self.install_dir.join("version.txt");

        if !python_path.exists() {
            return Err(format!(
                "Installation verification failed: {} not found. Archive may be empty or corrupted.",
                python_path.display()
            ));
        }
        if !main_py.exists() {
            return Err(format!(
                "Installation verification failed: {} not found. Archive may be corrupted.",
                main_py.display()
            ));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&python_path)
                .map_err(|e| format!("Failed to stat python binary: {}", e))?;
            if meta.permissions().mode() & 0o111 == 0 {
                fs::set_permissions(&python_path, fs::Permissions::from_mode(0o755))
                    .map_err(|e| format!("Failed to set python executable: {}", e))?;
            }
        }

        let smoke_test = std::process::Command::new(&python_path)
            .arg("--version")
            .env_remove("HSA_OVERRIDE_GFX_VERSION")
            .env_remove("HIP_VISIBLE_DEVICES")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match smoke_test {
            Ok(out) if out.status.success() => {
                let ver = String::from_utf8_lossy(&out.stdout);
                eprintln!("[Install] Python smoke test passed: {}", ver.trim());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                eprintln!(
                    "[Install] Warning: Python smoke test failed (exit {:?}). \
                     This may be due to missing ROCm runtime. Error: {}",
                    out.status.code(),
                    stderr.trim()
                );
            }
            Err(e) => {
                eprintln!("[Install] Warning: Could not execute python binary: {}", e);
            }
        }

        if !version_file.exists() {
            eprintln!("[Install] Warning: version.txt not found, installation may be incomplete");
        }

        Ok(())
    }
}

#[allow(dead_code)]
pub fn resolve_path(base: &Path, relative: &Path) -> PathBuf {
    normalize_path(&base.join(relative))
}

pub fn default_install_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/comfyui-desktop"))
}

pub fn backend_download_url() -> String {
    format!(
        "https://github.com/rakaarwaky/ComfyUi-arwaky/releases/download/backend-v{}/{}",
        BACKEND_VERSION, BACKEND_ARCHIVE_NAME
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // --- normalize_path ---

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

    // --- validate_symlink_target ---

    #[test]
    fn test_validate_absolute_rejected() {
        let base = Path::new("/tmp/install");
        let target = Path::new("/etc/passwd");
        assert!(validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_traversal_rejected() {
        let base = Path::new("/tmp/install");
        let target = Path::new("../../etc/passwd");
        assert!(validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_inner_traversal_rejected() {
        let base = Path::new("/tmp/install");
        let target = Path::new("subdir/../escape");
        assert!(validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_escape_via_normalization_rejected() {
        let base = Path::new("/tmp/install/dir");
        let target = Path::new("../outside");
        assert!(validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_valid_relative_ok() {
        let base = Path::new("/tmp/install");
        let target = Path::new("venv/bin/python3");
        assert!(validate_symlink_target(target, base).is_ok());
    }

    #[test]
    fn test_validate_same_dir_ok() {
        let base = Path::new("/tmp/install");
        let target = Path::new("file.txt");
        assert!(validate_symlink_target(target, base).is_ok());
    }

    // --- resolve_path ---

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

    // --- utilities ---

    #[test]
    fn test_default_install_dir_has_comfyui_desktop() {
        let dir = default_install_dir();
        assert!(dir.is_some());
        if let Some(path) = dir {
            let s = path.to_string_lossy();
            assert!(s.contains("comfyui-desktop"));
        }
    }

    #[test]
    fn test_backend_download_url_format() {
        let url = backend_download_url();
        assert!(url.contains("github.com/rakaarwaky"));
        assert!(url.contains(BACKEND_ARCHIVE_NAME));
        assert!(url.contains(BACKEND_VERSION));
    }

    #[test]
    fn test_backend_installer_new() {
        let dir = PathBuf::from("/tmp/test-install");
        let url = "https://example.com/archive.tar.gz".to_string();
        let installer = BackendInstaller::new(dir.clone(), url.clone(), None);
        assert_eq!(installer.install_dir, dir);
        assert_eq!(installer.archive_url, url);
        assert!(installer.expected_sha256.is_none());
    }

    #[test]
    fn test_is_installed_returns_false_for_nonexistent() {
        let dir = PathBuf::from("/tmp/nonexistent-comfyui-test");
        let installer = BackendInstaller::new(dir, String::new(), None);
        assert!(!installer.is_installed());
    }

    #[test]
    fn test_installed_version_returns_none_for_nonexistent() {
        let dir = PathBuf::from("/tmp/nonexistent-comfyui-test");
        let installer = BackendInstaller::new(dir, String::new(), None);
        assert!(installer.installed_version().is_none());
    }

    #[test]
    fn test_constants_sanity() {
        assert!(!BACKEND_VERSION.is_empty());
        assert!(BACKEND_ARCHIVE_NAME.ends_with(".tar.gz"));
        assert!(BACKEND_ARCHIVE_NAME.contains("comfyui-backend"));
    }
}
