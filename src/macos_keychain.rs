use security_framework::base::Error as SecurityError;
use security_framework::passwords::set_generic_password;

use crate::keychain::{KeychainError, Result};

fn map_security_error(error: SecurityError) -> KeychainError {
    KeychainError::KeychainFailure {
        code: error.code(),
        message: error.message(),
    }
}

pub(crate) fn save_generic_password(service: &str, env_name: &str, secret: &[u8]) -> Result<()> {
    set_generic_password(service, env_name, secret).map_err(map_security_error)
}
