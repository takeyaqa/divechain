use security_framework::base::Error as SecurityError;
use security_framework::os::macos::keychain::SecKeychain;

use crate::keychain::{KeychainError, Result};

fn map_security_error(error: SecurityError) -> KeychainError {
    KeychainError::KeychainFailure {
        code: error.code(),
        message: error.message(),
    }
}

fn keychain_service_name(namespace: &str) -> String {
    format!("divechain-{namespace}")
}

pub(crate) fn save_generic_password(namespace: &str, env_name: &str, secret: &[u8]) -> Result<()> {
    let service = keychain_service_name(namespace);

    SecKeychain::default()
        .map_err(map_security_error)?
        .set_generic_password(&service, env_name, secret)
        .map_err(map_security_error)
}
