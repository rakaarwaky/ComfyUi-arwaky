// PURPOSE: downloader-file-utils — infrastructure: filesystem adapter (implements FileValidationPort)

use std::fs;
use std::path::Path;

use downloader_shared::contract_file_port::FileValidationPort;

pub struct FsAdapter;

impl FileValidationPort for FsAdapter {
    fn file_exists_valid(&self, path: &Path, expected_size: u64, url: Option<&str>) -> bool {
        file_exists_valid(path, expected_size, url)
    }

    fn verify_sha256(&self, path: &Path, expected_hex: &str) -> bool {
        verify_sha256(path, expected_hex)
    }

    fn sanitize_filename(&self, filename: &str) -> String {
        sanitize_filename(filename)
    }

    fn get_available_space(&self, path: &Path) -> std::io::Result<u64> {
        get_available_space(path)
    }
}

pub fn file_exists_valid(path: &Path, expected_size: u64, url: Option<&str>) -> bool {
    if !path.exists() {
        return false;
    }
    if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            let actual_size = metadata.len();
            let is_size_valid = |expected: u64| -> bool {
                if expected == 0 {
                    return actual_size >= 1000;
                }
                let diff = actual_size.abs_diff(expected);
                let allowed_diff = (expected / 100).max(1024 * 1024);
                diff <= allowed_diff
            };
            if let Some(url_str) = url {
                if let Ok(cache) = crate::SIZE_CACHE.read() {
                    if let Some(&cached_size) = cache.sizes.get(url_str) {
                        if is_size_valid(cached_size) && expected_size == 0 {
                            return true;
                        }
                    }
                }
            }
            if expected_size == 0 {
                return actual_size >= 1000;
            }
            return is_size_valid(expected_size);
        }
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            return entries.count() > 5;
        }
    }
    false
}

pub fn verify_sha256(path: &Path, expected_hex: &str) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected_hex)
}

pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|&c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
        .collect::<String>()
        .replace("..", ".")
}

pub fn get_available_space(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let mut check_path = path.to_path_buf();
    while !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            break;
        }
    }
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let c_path = CString::new(check_path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // SAFETY: c_path is a valid CString (null-terminated, no interior null bytes).
    // stats is a MaybeUninit that will be written by the call.
    // We check return value — only read stats on success (res == 0).
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    // SAFETY: assume_init is safe only when statvfs returned 0 (success),
    // which guarantees stats has been fully written by the kernel.
    if res == 0 {
        let stats = unsafe { stats.assume_init() };
        Ok(stats.f_bavail * stats.f_frsize)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_normal_filename() {
        assert_eq!(sanitize_filename("model.safetensors"), "model.safetensors");
    }

    #[test]
    fn sanitize_removes_spaces() {
        assert_eq!(
            sanitize_filename("my model v2.safetensors"),
            "mymodelv2.safetensors"
        );
    }

    #[test]
    fn sanitize_removes_special_chars() {
        assert_eq!(
            sanitize_filename("flux1-dev (Q5_K_S).gguf"),
            "flux1-devQ5_K_S.gguf"
        );
    }

    #[test]
    fn sanitize_collapses_double_dot() {
        assert_eq!(sanitize_filename("test..safetensors"), "test.safetensors");
    }

    #[test]
    fn sanitize_triple_dot_becomes_double_dot() {
        assert_eq!(sanitize_filename("test...safetensors"), "test..safetensors");
    }

    #[test]
    fn sanitize_empty_string() {
        assert_eq!(sanitize_filename(""), "");
    }

    #[test]
    fn sanitize_allows_hyphen_underscore() {
        assert_eq!(sanitize_filename("my-model_v2.gguf"), "my-model_v2.gguf");
    }

    #[test]
    fn sanitize_replaces_path_separators() {
        let result = sanitize_filename("path/to/model.safetensors");
        assert!(!result.contains('/'));
    }
}
