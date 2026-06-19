// PURPOSE: BackendInstaller capability — implements BackendInstallProtocol.
// Stateless struct — all parameters passed to install().

use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

use launcher_shared::contract_backend_install_protocol::BackendInstallProtocol;
use launcher_shared::{
    normalize_path, ArchiveUrl, BackendInstallError, BackendInstallEvent, InstallDir, Sha256Hash,
};

const BACKEND_VERSION: &str = "1.1.0";
const BACKEND_ARCHIVE_NAME: &str = "comfyui-backend-linux-x86_64.tar.gz";

pub struct BackendInstaller;

impl BackendInstallProtocol for BackendInstaller {
    fn is_installed(&self, install_dir: &InstallDir) -> bool {
        install_dir.as_path().join("venv/bin/python").exists()
            && install_dir.as_path().join("ComfyUI/main.py").exists()
    }

    fn installed_version(&self, install_dir: &InstallDir) -> Option<String> {
        let version_file = install_dir.as_path().join("version.txt");
        fs::read_to_string(version_file)
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn default_install_dir(&self) -> Option<InstallDir> {
        std::env::var("HOME")
            .ok()
            .map(|home| InstallDir(PathBuf::from(home).join(".local/share/comfyui-desktop")))
    }

    fn backend_download_url(&self) -> ArchiveUrl {
        ArchiveUrl(format!(
            "https://github.com/rakaarwaky/ComfyUi-arwaky/releases/download/backend-v{}/{}",
            BACKEND_VERSION, BACKEND_ARCHIVE_NAME
        ))
    }

    fn expected_sha256(&self) -> Option<Sha256Hash> {
        Some(Sha256Hash(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        ))
    }

    fn install(
        &self,
        install_dir: &InstallDir,
        archive_url: &ArchiveUrl,
        expected_sha256: Option<&Sha256Hash>,
        cancel_token: &AtomicBool,
        on_event: &dyn Fn(BackendInstallEvent),
    ) -> Result<(), BackendInstallError> {
        if cancel_token.load(Ordering::Acquire) {
            return Err(BackendInstallError::DownloadCancelled);
        }

        fs::create_dir_all(install_dir.as_path())
            .map_err(|e| BackendInstallError::IoError(e.to_string()))?;

        BackendInstaller::check_disk_space(install_dir.as_path())
            .map_err(BackendInstallError::IoError)?;

        let tmp_dir = install_dir.as_path().join(".tmp");
        let archive_path = tmp_dir.join(BACKEND_ARCHIVE_NAME);

        if tmp_dir.exists() {
            fs::remove_dir_all(&tmp_dir)
                .map_err(|e| BackendInstallError::IoError(e.to_string()))?;
        }
        fs::create_dir_all(&tmp_dir).map_err(|e| BackendInstallError::IoError(e.to_string()))?;

        on_event(BackendInstallEvent::Downloading {
            bytes_downloaded: 0,
            total_bytes: 0,
        });

        BackendInstaller::download_archive(
            &archive_path,
            archive_url.as_str(),
            on_event,
            cancel_token,
        )
        .map_err(BackendInstallError::ConnectionFailed)?;

        if let Some(expected) = expected_sha256 {
            on_event(BackendInstallEvent::Verifying);
            BackendInstaller::verify_checksum(&archive_path, expected.as_str()).map_err(|e| {
                BackendInstallError::ChecksumMismatch {
                    expected: expected.0.clone(),
                    computed: e,
                }
            })?;
        }

        on_event(BackendInstallEvent::Extracting { files_extracted: 0 });

        BackendInstaller::extract_archive(
            install_dir.as_path(),
            &archive_path,
            on_event,
            cancel_token,
        )
        .map_err(BackendInstallError::ExtractionFailed)?;

        BackendInstaller::finalize_installation(install_dir.as_path())
            .map_err(BackendInstallError::VerificationFailed)?;

        fs::write(install_dir.as_path().join("version.txt"), BACKEND_VERSION)
            .map_err(|e| BackendInstallError::IoError(e.to_string()))?;

        let _ = fs::remove_dir_all(&tmp_dir);

        on_event(BackendInstallEvent::Complete);

        Ok(())
    }
}

impl BackendInstaller {
    fn check_disk_space(install_dir: &Path) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let output = std::process::Command::new("df")
                .arg("-B1")
                .arg("--output=avail")
                .arg(install_dir)
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

    fn download_archive(
        dest: &Path,
        archive_url: &str,
        on_event: &dyn Fn(BackendInstallEvent),
        cancel_token: &AtomicBool,
    ) -> Result<(), String> {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(std::time::Duration::from_secs(30)))
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build(),
        );

        let response = agent.get(archive_url).call().map_err(|e| match e {
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
                on_event(BackendInstallEvent::Downloading {
                    bytes_downloaded,
                    total_bytes,
                });
                last_report = std::time::Instant::now();
            }
        }

        writer
            .flush()
            .map_err(|e| format!("Failed to flush download: {}", e))?;

        Ok(())
    }

    fn verify_checksum(file_path: &Path, expected: &str) -> Result<(), String> {
        let file = fs::File::open(file_path)
            .map_err(|e| format!("Failed to open archive for verification: {}", e))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();

        let mut buf = vec![0u8; 256 * 1024];
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

    fn extract_archive(
        install_dir: &Path,
        archive_path: &Path,
        on_event: &dyn Fn(BackendInstallEvent),
        cancel_token: &AtomicBool,
    ) -> Result<(), String> {
        let file =
            fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let tmp_extract = install_dir.join(".tmp_extract");
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

                    if let Err(e) = Self::validate_symlink_target(&link_target, &tmp_extract) {
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
            if extracted.is_multiple_of(100) {
                on_event(BackendInstallEvent::Extracting {
                    files_extracted: extracted,
                });
            }
        }

        Ok(())
    }

    fn validate_symlink_target(target: &Path, base_dir: &Path) -> Result<(), String> {
        if target.is_absolute() {
            return Err(format!("Absolute symlink rejected: {}", target.display()));
        }

        for component in target.components() {
            if matches!(component, std::path::Component::ParentDir) {
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

    fn finalize_installation(install_dir: &Path) -> Result<(), String> {
        let tmp_extract = install_dir.join(".tmp_extract");

        if tmp_extract.exists() {
            for entry in fs::read_dir(&tmp_extract)
                .map_err(|e| format!("Failed to read extracted directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let src = entry.path();
                let file_name = entry.file_name();
                let dest = install_dir.join(&file_name);

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

        let python_path = install_dir.join("venv/bin/python");
        let main_py = install_dir.join("ComfyUI/main.py");
        let version_file = install_dir.join("version.txt");

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_validate_absolute_rejected() {
        let base = Path::new("/tmp/install");
        let target = Path::new("/etc/passwd");
        assert!(BackendInstaller::validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_traversal_rejected() {
        let base = Path::new("/tmp/install");
        let target = Path::new("../../etc/passwd");
        assert!(BackendInstaller::validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_escape_via_normalization_rejected() {
        let base = Path::new("/tmp/install/dir");
        let target = Path::new("../outside");
        assert!(BackendInstaller::validate_symlink_target(target, base).is_err());
    }

    #[test]
    fn test_validate_valid_relative_ok() {
        let base = Path::new("/tmp/install");
        let target = Path::new("venv/bin/python3");
        assert!(BackendInstaller::validate_symlink_target(target, base).is_ok());
    }

    #[test]
    fn test_default_install_dir_has_comfyui_desktop() {
        let installer = BackendInstaller;
        let dir = installer.default_install_dir();
        assert!(dir.is_some());
        if let Some(path) = dir {
            assert!(path.0.to_string_lossy().contains("comfyui-desktop"));
        }
    }

    #[test]
    fn test_backend_download_url_format() {
        let installer = BackendInstaller;
        let url = installer.backend_download_url();
        assert!(url.0.contains("github.com/rakaarwaky"));
        assert!(url.0.contains(BACKEND_ARCHIVE_NAME));
        assert!(url.0.contains(BACKEND_VERSION));
    }

    #[test]
    fn test_is_installed_returns_false_for_nonexistent() {
        let installer = BackendInstaller;
        let p = Path::new("/tmp/nonexistent-comfyui-test");
        let dir = InstallDir(p.to_path_buf());
        assert!(!installer.is_installed(&dir));
    }

    #[test]
    fn test_installed_version_returns_none_for_nonexistent() {
        let installer = BackendInstaller;
        let p = Path::new("/tmp/nonexistent-comfyui-test");
        let dir = InstallDir(p.to_path_buf());
        assert!(installer.installed_version(&dir).is_none());
    }
}
