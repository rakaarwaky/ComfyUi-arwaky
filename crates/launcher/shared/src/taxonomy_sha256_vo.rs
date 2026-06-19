// PURPOSE: Value object for SHA256 checksum hash.

#[derive(Debug, Clone, PartialEq)]
pub struct Sha256Hash(pub String);

impl Sha256Hash {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Sha256Hash {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string() {
        let h = Sha256Hash::from(String::from("abc"));
        assert_eq!(h.0, "abc");
    }
    #[test]
    fn as_str_returns_inner() {
        assert_eq!(Sha256Hash("a".into()).as_str(), "a");
    }
    #[test]
    fn partial_eq_same() {
        assert_eq!(Sha256Hash("a".into()), Sha256Hash("a".into()));
    }
    #[test]
    fn partial_eq_different() {
        assert_ne!(Sha256Hash("a".into()), Sha256Hash("b".into()));
    }
}
