use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, KeychainError>;

#[derive(Debug)]
pub enum KeychainError {
    KeychainFailure { code: i32, message: Option<String> },
    SecretNotFound { namespace: String, env: String },
    UnsupportedPlatform(&'static str),
    Io(io::Error),
}

impl fmt::Display for KeychainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::SecretNotFound { namespace, env } => {
                write!(f, "secret '{namespace}.{env}' does not exist")
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

    pub fn list_namespaces(self) -> Result<Vec<String>> {
        backend::list_namespaces()
    }

    pub fn load_namespace_env(self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        backend::load_namespace_env(namespace)
    }

    pub fn save_generic_password(self, namespace: &str, env: &str, secret: &[u8]) -> Result<()> {
        backend::save_generic_password(namespace, env, secret)
    }

    pub fn delete_generic_password(self, namespace: &str, env: &str) -> Result<()> {
        backend::delete_generic_password(namespace, env)
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use crate::macos_keychain;

    use super::Result;

    pub(super) fn list_namespaces() -> Result<Vec<String>> {
        macos_keychain::list_namespaces()
    }

    pub(super) fn load_namespace_env(namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        macos_keychain::load_namespace_env(namespace)
    }

    pub(super) fn save_generic_password(namespace: &str, env: &str, secret: &[u8]) -> Result<()> {
        macos_keychain::save_generic_password(namespace, env, secret)
    }

    pub(super) fn delete_generic_password(namespace: &str, env: &str) -> Result<()> {
        macos_keychain::delete_generic_password(namespace, env)
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::{KeychainError, Result};

    pub(super) fn list_namespaces() -> Result<Vec<String>> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn load_namespace_env(_namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn save_generic_password(
        _namespace: &str,
        _env: &str,
        _secret: &[u8],
    ) -> Result<()> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn delete_generic_password(_namespace: &str, _env: &str) -> Result<()> {
        Err(KeychainError::UnsupportedPlatform(std::env::consts::OS))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "macos"))]
    use super::KeychainError;
    use super::KeychainStore;

    #[cfg(target_os = "macos")]
    #[test]
    fn store_is_constructible() {
        let _ = KeychainStore::new();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn backend_is_disabled_outside_macos() {
        let error = KeychainStore::new()
            .save_generic_password("namespace", "ENV_NAME", b"secret")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            KeychainError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {other:?}"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn list_namespaces_is_disabled_outside_macos() {
        let error = KeychainStore::new()
            .list_namespaces()
            .expect_err("non-mac targets should reject keychain access");

        match error {
            KeychainError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {other:?}"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn load_namespace_env_is_disabled_outside_macos() {
        let error = KeychainStore::new()
            .load_namespace_env("namespace")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            KeychainError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {other:?}"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn delete_generic_password_is_disabled_outside_macos() {
        let error = KeychainStore::new()
            .delete_generic_password("namespace", "ENV_NAME")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            KeychainError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {other:?}"),
        }
    }
}
