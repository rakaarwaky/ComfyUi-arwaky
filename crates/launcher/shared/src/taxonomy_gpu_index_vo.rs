// PURPOSE: Value object for GPU index (which GPU to use for ROCm).

#[derive(Debug, Clone)]
pub struct GpuIndex(pub String);

impl GpuIndex {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_inner() {
        let idx = GpuIndex("0".to_string());
        assert_eq!(idx.as_str(), "0");
    }

    #[test]
    fn debug_format() {
        let idx = GpuIndex("1".to_string());
        assert!(format!("{:?}", idx).contains("1"));
    }
}
