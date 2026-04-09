use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, KeychainError>;

#[derive(Debug)]
pub enum KeychainError {
    NotFound { service: String, account: String },
    KeychainFailure { code: i32, message: Option<String> },
    UnsupportedPlatform(&'static str),
    Io(io::Error),
}

impl fmt::Display for KeychainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { service, account } => {
                write!(
                    f,
                    "no keychain item found for service={service:?}, account={account:?}"
                )
            }
            Self::KeychainFailure { code, message } => {
                if let Some(message) = message {
                    write!(
                        f,
                        "keychain operation failed with OSStatus {code}: {message}"
                    )
                } else {
                    write!(f, "keychain operation failed with OSStatus {code}")
                }
            }
            Self::UnsupportedPlatform(platform) => {
                write!(f, "macOS Keychain backend is unsupported on {platform}")
            }
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for KeychainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for KeychainError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> Self {
        Self
    }

    pub fn save_generic_password(self, service: &str, account: &str, secret: &[u8]) -> Result<()> {
        backend::save_generic_password(service, account, secret)
    }

    pub fn get_generic_password(self, service: &str, account: &str) -> Result<Vec<u8>> {
        backend::get_generic_password(service, account)
    }

    pub fn delete_generic_password(self, service: &str, account: &str) -> Result<()> {
        backend::delete_generic_password(service, account)
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use crate::macos_keychain;

    use super::Result;

    pub(super) fn save_generic_password(service: &str, account: &str, secret: &[u8]) -> Result<()> {
        macos_keychain::save_generic_password(service, account, secret)
    }

    pub(super) fn get_generic_password(service: &str, account: &str) -> Result<Vec<u8>> {
        macos_keychain::get_generic_password(service, account)
    }

    pub(super) fn delete_generic_password(service: &str, account: &str) -> Result<()> {
        macos_keychain::delete_generic_password(service, account)
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::{KeychainError, Result};

    pub(super) fn save_generic_password(
        _service: &str,
        _account: &str,
        _secret: &[u8],
    ) -> Result<()> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn get_generic_password(_service: &str, _account: &str) -> Result<Vec<u8>> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn delete_generic_password(_service: &str, _account: &str) -> Result<()> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }
}

#[cfg(test)]
mod tests {
    use super::{KeychainError, KeychainStore};

    #[cfg(target_os = "macos")]
    mod macos {
        use std::process;
        use std::time::{SystemTime, UNIX_EPOCH};

        use super::{KeychainError, KeychainStore};

        struct TestEntry {
            service: String,
            account: String,
        }

        impl TestEntry {
            fn new(label: &str) -> Self {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock before unix epoch")
                    .as_nanos();
                let suffix = format!("pid{}-{nanos}", process::id());

                Self {
                    service: format!("dev.takeyaqa.divechain.test.{label}.{suffix}"),
                    account: format!("account.{suffix}"),
                }
            }
        }

        impl Drop for TestEntry {
            fn drop(&mut self) {
                let _ = KeychainStore::new().delete_generic_password(&self.service, &self.account);
            }
        }

        #[test]
        #[ignore = "requires an authorized macOS keychain session"]
        fn round_trip_generic_password() {
            let entry = TestEntry::new("roundtrip");
            let secret = b"roundtrip-secret";
            let store = KeychainStore::new();

            store
                .save_generic_password(&entry.service, &entry.account, secret)
                .expect("save should succeed");
            let loaded = store
                .get_generic_password(&entry.service, &entry.account)
                .expect("load should succeed");

            assert_eq!(loaded, secret);
        }

        #[test]
        #[ignore = "requires an authorized macOS keychain session"]
        fn save_updates_existing_password() {
            let entry = TestEntry::new("update");
            let store = KeychainStore::new();

            store
                .save_generic_password(&entry.service, &entry.account, b"first-secret")
                .expect("initial save should succeed");
            store
                .save_generic_password(&entry.service, &entry.account, b"second-secret")
                .expect("overwrite should succeed");

            let loaded = store
                .get_generic_password(&entry.service, &entry.account)
                .expect("load should succeed");

            assert_eq!(loaded, b"second-secret");
        }

        #[test]
        #[ignore = "requires an authorized macOS keychain session"]
        fn delete_removes_existing_password() {
            let entry = TestEntry::new("delete");
            let store = KeychainStore::new();

            store
                .save_generic_password(&entry.service, &entry.account, b"delete-me")
                .expect("save should succeed");
            store
                .delete_generic_password(&entry.service, &entry.account)
                .expect("delete should succeed");

            let error = store
                .get_generic_password(&entry.service, &entry.account)
                .expect_err("entry should be gone");

            match error {
                KeychainError::NotFound { service, account } => {
                    assert_eq!(service, entry.service);
                    assert_eq!(account, entry.account);
                }
                other => panic!("expected not found error, got {other:?}"),
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn backend_is_disabled_outside_macos() {
        let error = KeychainStore::new()
            .get_generic_password("service", "account")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            KeychainError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {other:?}"),
        }
    }
}
