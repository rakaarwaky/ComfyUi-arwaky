use std::fs;
use std::path::Path;

pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "~12 GiB".to_string();
    }
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn file_exists_valid(path: &Path, expected_size: u64) -> bool {
    if !path.exists() {
        return false;
    }
    if expected_size == 0 {
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                return entries.count() > 5;
            }
        }
        return path.exists();
    }
    if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            let actual_size = metadata.len();
            if expected_size <= 1_000_000 {
                actual_size >= 1000
            } else {
                let min_allowed = (expected_size as f64 * 0.95) as u64;
                actual_size >= min_allowed
            }
        } else {
            false
        }
    } else {
        false
    }
}

pub fn get_available_space(path: &Path) -> std::io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // Find the first ancestor that actually exists
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
    
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if res == 0 {
        let stats = unsafe { stats.assume_init() };
        Ok(stats.f_bavail as u64 * stats.f_frsize as u64)
    } else {
        Err(std::io::Error::last_os_error())
    }
}
