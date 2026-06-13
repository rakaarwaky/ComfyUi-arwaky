use std::path::Path;

pub fn detect_dgpu_index() -> String {
    let output = std::process::Command::new("rocm-smi")
        .arg("--showmeminfo")
        .arg("vram")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut max_vram = 0u64;
            let mut best_gpu = "0".to_string();

            for line in stdout.lines() {
                if line.contains("VRAM Total Memory") {
                    if let (Some(start_idx), Some(end_idx)) = (line.find('['), line.find(']')) {
                        let gpu_num = &line[start_idx + 1..end_idx];
                        if let Some(col_idx) = line.rfind(':') {
                            let vram_str = line[col_idx + 1..].trim();
                            if let Ok(vram_bytes) = vram_str.parse::<u64>() {
                                eprintln!(
                                    "[GPU Detection] GPU {}: {} bytes VRAM",
                                    gpu_num, vram_bytes
                                );
                                if vram_bytes > max_vram {
                                    max_vram = vram_bytes;
                                    best_gpu = gpu_num.to_string();
                                }
                            } else {
                                eprintln!(
                                    "[GPU Detection] Failed to parse VRAM value: '{}'",
                                    vram_str
                                );
                            }
                        }
                    }
                }
            }
            best_gpu
        }
        Ok(out) => {
            eprintln!(
                "rocm-smi exited with error: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            "0".to_string()
        }
        Err(e) => {
            eprintln!("rocm-smi not found or failed to execute: {:?}", e);
            "0".to_string()
        }
    }
}

/// Detect if the GPU needs an HSA_OVERRIDE_GFX_VERSION environment variable.
/// Uses sysfs topology nodes first, falls back to rocminfo.
pub fn detect_hsa_override() -> Option<&'static str> {
    let topology_base = Path::new("/sys/class/kfd/kfd/topology/nodes");

    if topology_base.exists() {
        if let Ok(entries) = std::fs::read_dir(topology_base) {
            for entry in entries.flatten() {
                let gfx_path = entry.path().join("gfx_target_version");
                if let Ok(content) = std::fs::read_to_string(&gfx_path) {
                    let ver_str = content.trim();
                    if ver_str.is_empty() || ver_str == "0" {
                        continue;
                    }
                    if let Ok(version) = ver_str.parse::<u32>() {
                        let major = version / 10000;
                        let _minor = (version / 100) % 100;
                        let patch = version % 100;

                        eprintln!(
                            "[GPU Detection] sysfs node {}: gfx_target_version={} → gfx{}.{}.{}",
                            entry.file_name().to_string_lossy(),
                            version,
                            major,
                            _minor,
                            patch
                        );

                        if patch > 0 {
                            return match major {
                                10 => Some("10.3.0"),
                                11 => Some("11.0.0"),
                                _ => None,
                            };
                        }
                        return None;
                    }
                }
            }
        }
        return None;
    }

    detect_hsa_override_fallback()
}

fn detect_hsa_override_fallback() -> Option<&'static str> {
    let output = std::process::Command::new("rocminfo").output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if let Some(pos) = line.rfind("gfx") {
                    let version: String = line[pos + 3..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if !version.is_empty() {
                        return parse_hsa_override(&version);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_hsa_override(gfx: &str) -> Option<&'static str> {
    if gfx.len() == 4 && gfx.chars().all(|c| c.is_ascii_digit()) {
        let major = gfx.get(0..2).unwrap_or("");
        let patch = gfx.get(3..4).unwrap_or("");
        if patch != "0" {
            return match major {
                "10" => Some("10.3.0"),
                "11" => Some("11.0.0"),
                "12" => Some("12.0.0"), // RDNA4 future-proof
                _ => None,              // Unknown major: don't guess
            };
        }
        return None;
    }

    let parts: Vec<&str> = gfx.split('.').collect();
    if parts.len() == 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            if patch > 0 {
                let major = parts[0].parse::<u32>().unwrap_or(10);
                return match major {
                    10 => Some("10.3.0"),
                    11 => Some("11.0.0"),
                    12 => Some("12.0.0"), // RDNA4
                    _ => None,            // Unknown major: safe default
                };
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_hsa_override ---

    #[test]
    fn test_parse_rdna2_native() {
        assert_eq!(parse_hsa_override("1030"), None);
    }

    #[test]
    fn test_parse_rdna2_patched() {
        assert_eq!(parse_hsa_override("1031"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_rdna2_other_patches() {
        assert_eq!(parse_hsa_override("1032"), Some("10.3.0"));
        assert_eq!(parse_hsa_override("1033"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_rdna3_1100_native() {
        assert_eq!(parse_hsa_override("1100"), None);
    }

    #[test]
    fn test_parse_rdna3_1101_patched() {
        assert_eq!(parse_hsa_override("1101"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_rdna3_1102() {
        assert_eq!(parse_hsa_override("1102"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_rdna3_1150_native() {
        assert_eq!(parse_hsa_override("1150"), None);
    }

    #[test]
    fn test_parse_rdna3_1151_patched() {
        assert_eq!(parse_hsa_override("1151"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_dotted_rdna2() {
        assert_eq!(parse_hsa_override("10.3.1"), Some("10.3.0"));
    }

    #[test]
    fn test_parse_dotted_rdna3() {
        assert_eq!(parse_hsa_override("11.0.1"), Some("11.0.0"));
    }

    #[test]
    fn test_parse_dotted_unknown_major() {
        assert_eq!(parse_hsa_override("12.0.1"), Some("12.0.0"));
        assert_eq!(parse_hsa_override("9.0.1"), None);
    }

    #[test]
    fn test_parse_unknown_major_returns_none() {
        assert_eq!(parse_hsa_override("9931"), None);
        assert_eq!(parse_hsa_override("1301"), None);
    }

    #[test]
    fn test_parse_rdna4_future() {
        assert_eq!(parse_hsa_override("1200"), None);
        assert_eq!(parse_hsa_override("1201"), Some("12.0.0"));
        assert_eq!(parse_hsa_override("12.0.1"), Some("12.0.0"));
    }

    #[test]
    fn test_parse_dotted_no_patch() {
        assert_eq!(parse_hsa_override("10.3.0"), None);
        assert_eq!(parse_hsa_override("11.0.0"), None);
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_hsa_override("invalid"), None);
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_hsa_override(""), None);
    }

    #[test]
    fn test_parse_short() {
        assert_eq!(parse_hsa_override("103"), None);
    }
}
