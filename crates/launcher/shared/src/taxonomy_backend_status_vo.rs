// PURPOSE: Backend install status — domain enum for installed state check.

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub enum BackendStatus {
    Installed { version: Option<String> },
    CustomInstall,
    NotInstalled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_eq_installed() { assert_eq!(BackendStatus::Installed { version: Some("1.0".into()) }, BackendStatus::Installed { version: Some("1.0".into()) }); }
    #[test]
    fn partial_eq_installed_different() { assert_ne!(BackendStatus::Installed { version: Some("1.0".into()) }, BackendStatus::Installed { version: Some("2.0".into()) }); }
    #[test]
    fn partial_eq_custom() { assert_eq!(BackendStatus::CustomInstall, BackendStatus::CustomInstall); }
    #[test]
    fn partial_eq_not_installed() { assert_eq!(BackendStatus::NotInstalled, BackendStatus::NotInstalled); }
    #[test]
    fn serde_roundtrip_installed() { let s = BackendStatus::Installed { version: Some("1.0".into()) }; let j = serde_json::to_string(&s).unwrap(); let d: BackendStatus = serde_json::from_str(&j).unwrap(); assert_eq!(s, d); }
    #[test]
    fn serde_roundtrip_custom() { let j = serde_json::to_string(&BackendStatus::CustomInstall).unwrap(); let d: BackendStatus = serde_json::from_str(&j).unwrap(); assert_eq!(BackendStatus::CustomInstall, d); }
}
