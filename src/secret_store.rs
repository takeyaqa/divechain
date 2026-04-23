use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SecretStoreError>;

#[derive(Debug, Error)]
pub enum SecretStoreError {
    #[error("keychain operation failed with OSStatus {}: {}", code, message.as_deref().unwrap_or("no message"))]
    BackendFailure { code: i32, message: Option<String> },
    #[error("namespace '{}' does not exist", namespace)]
    NamespaceNotFound { namespace: String },
    #[error("secret '{}.{}' does not exist", namespace, env)]
    SecretNotFound { namespace: String, env: String },
    #[error("macOS Keychain backend is unsupported on {}", .0)]
    UnsupportedPlatform(&'static str),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SecretStore;

impl SecretStore {
    pub fn new() -> Self {
        Self
    }

    pub fn save_secret(self, namespace: &str, env: &str, secret: &[u8]) -> Result<()> {
        backend::save_secret(namespace, env, secret)
    }

    pub fn delete_secret(self, namespace: &str, env: &str) -> Result<()> {
        backend::delete_secret(namespace, env)
    }

    pub fn list_namespaces(self) -> Result<Vec<String>> {
        backend::list_namespaces()
    }

    pub fn load_namespace_env(self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        backend::load_namespace_env(namespace)
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use crate::macos_keychain;

    use super::Result;

    pub(super) fn save_secret(namespace: &str, env: &str, secret: &[u8]) -> Result<()> {
        macos_keychain::save_secret(namespace, env, secret)
    }

    pub(super) fn delete_secret(namespace: &str, env: &str) -> Result<()> {
        macos_keychain::delete_secret(namespace, env)
    }

    pub(super) fn list_namespaces() -> Result<Vec<String>> {
        macos_keychain::list_namespaces()
    }

    pub(super) fn load_namespace_env(namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        macos_keychain::load_namespace_env(namespace)
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::{Result, SecretStoreError};

    pub(super) fn save_secret(_namespace: &str, _env: &str, _secret: &[u8]) -> Result<()> {
        Err(SecretStoreError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn delete_secret(_namespace: &str, _env: &str) -> Result<()> {
        Err(SecretStoreError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn list_namespaces() -> Result<Vec<String>> {
        Err(SecretStoreError::UnsupportedPlatform(std::env::consts::OS))
    }

    pub(super) fn load_namespace_env(_namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        Err(SecretStoreError::UnsupportedPlatform(std::env::consts::OS))
    }
}

#[cfg(test)]
mod tests {
    use super::SecretStore;
    use super::SecretStoreError;

    #[test]
    fn namespace_not_found_error_formats_cleanly() {
        let error = SecretStoreError::NamespaceNotFound {
            namespace: "aws".to_owned(),
        };

        assert_eq!(error.to_string(), "namespace 'aws' does not exist");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn store_is_constructible() {
        let _ = SecretStore::new();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn backend_is_disabled_outside_macos() {
        let error = SecretStore::new()
            .save_secret("namespace", "ENV_NAME", b"secret")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            SecretStoreError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {:?}", other),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn list_namespaces_is_disabled_outside_macos() {
        let error = SecretStore::new()
            .list_namespaces()
            .expect_err("non-mac targets should reject keychain access");

        match error {
            SecretStoreError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {:?}", other),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn load_namespace_env_is_disabled_outside_macos() {
        let error = SecretStore::new()
            .load_namespace_env("namespace")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            SecretStoreError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {:?}", other),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn delete_secret_is_disabled_outside_macos() {
        let error = SecretStore::new()
            .delete_secret("namespace", "ENV_NAME")
            .expect_err("non-mac targets should reject keychain access");

        match error {
            SecretStoreError::UnsupportedPlatform(platform) => {
                assert_eq!(platform, std::env::consts::OS);
            }
            other => panic!("expected unsupported platform, got {:?}", other),
        }
    }
}
