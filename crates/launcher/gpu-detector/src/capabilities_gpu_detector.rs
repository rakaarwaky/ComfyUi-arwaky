// PURPOSE: GPU detection capability — implements GpuDetectionProtocol.

use launcher_shared::contract_gpu_detection_protocol::GpuDetectionProtocol;
use launcher_shared::{GpuError, GpuIndex, HsaOverride};

pub struct GpuDetector;

fn get_rocm_path() -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
    paths.insert(0, std::path::PathBuf::from("/opt/rocm/bin"));
    paths.insert(0, std::path::PathBuf::from("/opt/rocm-7.2.4/bin"));
    std::env::join_paths(paths).unwrap_or(path)
}

impl GpuDetectionProtocol for GpuDetector {
    fn detect_dgpu_index(&self) -> Result<GpuIndex, GpuError> {
        let output = std::process::Command::new("rocm-smi")
            .env("PATH", get_rocm_path())
            .arg("--showmeminfo").arg("vram").output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut max_vram = 0u64;
                let mut best_gpu = "0".to_string();
                for line in stdout.lines() {
                    if line.contains("VRAM Total Memory") {
                        if let (Some(start), Some(end)) = (line.find('['), line.find(']')) {
                            let gpu = &line[start + 1..end];
                            if let Some(col) = line.rfind(':') {
                                if let Ok(vram) = line[col + 1..].trim().parse::<u64>() {
                                    if vram > max_vram { max_vram = vram; best_gpu = gpu.to_string(); }
                                }
                            }
                        }
                    }
                }
                if best_gpu != "0" {
                    Ok(GpuIndex(best_gpu))
                } else {
                    Ok(GpuIndex("0".to_string()))
                }
            }
            _ => Err(GpuError::RocmNotAvailable),
        }
    }

    fn detect_hsa_override(&self) -> Result<Option<HsaOverride>, GpuError> {
        let topology = std::path::Path::new("/sys/class/kfd/kfd/topology/nodes");
        if !topology.exists() {
            return Ok(None);
        }
        let entries = std::fs::read_dir(topology)
            .map_err(|e| GpuError::HsaDetectionFailed(e.to_string()))?;
        for entry in entries.flatten() {
            let gfx_path = entry.path().join("gfx_target_version");
            if let Ok(content) = std::fs::read_to_string(&gfx_path) {
                let ver = content.trim();
                if ver.is_empty() || ver == "0" { continue; }
                if let Ok(v) = ver.parse::<u32>() {
                    let patch = v % 100;
                    if patch > 0 {
                        return Ok(match v / 10000 {
                            10 => Some(HsaOverride("10.3.0")),
                            11 => Some(HsaOverride("11.0.0")),
                            _ => None,
                        });
                    }
                    return Ok(None);
                }
            }
        }

        // Fallback: try rocm-smi --showhw
        let smi_output = std::process::Command::new("rocm-smi")
            .env("PATH", get_rocm_path())
            .arg("--showhw").output();
        if let Ok(out) = smi_output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    if let Some(pos) = line.rfind("gfx") {
                        let version: String = line[pos + 3..]
                            .chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                        if !version.is_empty() {
                            return Ok(Self::parse_hsa_override(&version));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}

impl GpuDetector {
    fn parse_hsa_override(gfx: &str) -> Option<HsaOverride> {
        // Format: "1030" (native), "1031" (patched → needs HSA_OVERRIDE)
        if gfx.len() >= 4 {
            let major = gfx.get(0..2).unwrap_or("");
            let patch = gfx.get(3..4).unwrap_or("");
            if patch != "0" {
                return match major {
                    "10" => Some(HsaOverride("10.3.0")),
                    "11" => Some(HsaOverride("11.0.0")),
                    "12" => Some(HsaOverride("12.0.0")),
                    _ => None,
                };
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rdna2_native() { assert_eq!(GpuDetector::parse_hsa_override("1030"), None); }
    #[test]
    fn test_parse_rdna2_patched() { assert_eq!(GpuDetector::parse_hsa_override("1031"), Some(HsaOverride("10.3.0"))); }
    #[test]
    fn test_parse_rdna3_1100_native() { assert_eq!(GpuDetector::parse_hsa_override("1100"), None); }
    #[test]
    fn test_parse_rdna3_1101_patched() { assert_eq!(GpuDetector::parse_hsa_override("1101"), Some(HsaOverride("11.0.0"))); }
    #[test]
    fn test_parse_invalid() { assert_eq!(GpuDetector::parse_hsa_override("invalid"), None); }
    #[test]
    fn test_parse_empty() { assert_eq!(GpuDetector::parse_hsa_override(""), None); }
}
