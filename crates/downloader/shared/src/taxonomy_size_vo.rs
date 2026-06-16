/// Pure function — format bytes to human-readable string. No IO.
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "Unknown".to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "Unknown");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kib() {
        assert_eq!(format_size(2048), "2.00 KiB");
    }

    #[test]
    fn test_format_size_mib() {
        assert_eq!(format_size(5 * 1024 * 1024), "5.00 MiB");
    }

    #[test]
    fn test_format_size_gib() {
        let val = 2 * 1024 * 1024 * 1024;
        assert_eq!(format_size(val), "2.00 GiB");
    }
}
