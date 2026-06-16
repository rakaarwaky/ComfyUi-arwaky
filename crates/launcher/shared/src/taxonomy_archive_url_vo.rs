// PURPOSE: Value object for backend archive download URL.

#[derive(Debug, Clone)]
pub struct ArchiveUrl(pub String);

impl ArchiveUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ArchiveUrl {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string() {
        let url = ArchiveUrl::from("https://example.com".to_string());
        assert_eq!(url.0, "https://example.com");
    }

    #[test]
    fn as_str_returns_inner() {
        let url = ArchiveUrl("https://test".to_string());
        assert_eq!(url.as_str(), "https://test");
    }

    #[test]
    fn debug_format() {
        let url = ArchiveUrl("x".to_string());
        assert!(format!("{:?}", url).contains("x"));
    }
}
