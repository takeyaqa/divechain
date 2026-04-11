use crate::keychain::{KeychainError, Result};

#[cfg(target_os = "macos")]
use security_framework::base::Error as SecurityError;
#[cfg(target_os = "macos")]
use security_framework::passwords::{PasswordOptions, set_generic_password_options};

const KEYCHAIN_SERVICE_PREFIX: &str = "divechain-";
const KEYCHAIN_ITEM_LABEL: &str = "divechain";

fn keychain_service_name(namespace: &str) -> String {
    format!("{KEYCHAIN_SERVICE_PREFIX}{namespace}")
}

#[cfg(target_os = "macos")]
fn map_security_error(error: SecurityError) -> KeychainError {
    KeychainError::KeychainFailure {
        code: error.code(),
        message: error.message(),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn save_generic_password(namespace: &str, env_name: &str, secret: &[u8]) -> Result<()> {
    let service = keychain_service_name(namespace);
    let mut options = PasswordOptions::new_generic_password(&service, env_name);
    options.set_label(KEYCHAIN_ITEM_LABEL);

    set_generic_password_options(secret, options).map_err(map_security_error)
}

#[cfg(test)]
mod tests {
    use super::keychain_service_name;

    #[test]
    fn prefixes_namespace_in_keychain_service_name() {
        assert_eq!(keychain_service_name("aws"), "divechain-aws");
    }
}
