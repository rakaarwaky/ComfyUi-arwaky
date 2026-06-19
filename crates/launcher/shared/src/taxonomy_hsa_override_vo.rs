// PURPOSE: Value object for HSA override version string (e.g. "10.3.0").

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HsaOverride(pub &'static str);

impl HsaOverride {
    pub fn as_str(&self) -> &str {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_returns_inner() {
        assert_eq!(HsaOverride("10.3.0").as_str(), "10.3.0");
    }
    #[test]
    fn partial_eq_same() {
        assert_eq!(HsaOverride("10.3.0"), HsaOverride("10.3.0"));
    }
    #[test]
    fn partial_eq_different() {
        assert_ne!(HsaOverride("10.3.0"), HsaOverride("11.0.0"));
    }
    #[test]
    fn copy_works() {
        let a = HsaOverride("x");
        let b = a;
        assert_eq!(a, b);
    }
    #[test]
    fn debug_format() {
        assert!(format!("{:?}", HsaOverride("x")).contains("x"));
    }
}
