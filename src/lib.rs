mod keychain;
#[cfg(target_os = "macos")]
mod macos_keychain;

pub use keychain::{KeychainError, KeychainStore, Result};
