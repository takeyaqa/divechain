use security_framework::base::Error as SecurityError;
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::os::macos::passwords::find_generic_password;
use security_framework_sys::base::errSecItemNotFound;

use crate::keychain::{KeychainError, Result};

fn map_security_error(error: SecurityError, service: &str, account: &str) -> KeychainError {
    if error.code() == errSecItemNotFound {
        return KeychainError::NotFound {
            service: service.to_owned(),
            account: account.to_owned(),
        };
    }

    KeychainError::KeychainFailure {
        code: error.code(),
        message: error.message(),
    }
}

fn find_password_item(
    keychain: Option<&SecKeychain>,
    service: &str,
    account: &str,
) -> Result<(
    security_framework::os::macos::passwords::SecKeychainItemPassword,
    security_framework::os::macos::keychain_item::SecKeychainItem,
)> {
    let keychains = keychain.map(std::slice::from_ref);

    find_generic_password(keychains, service, account)
        .map_err(|error| map_security_error(error, service, account))
}

fn save_generic_password_impl(
    keychain: Option<&SecKeychain>,
    service: &str,
    account: &str,
    secret: &[u8],
) -> Result<()> {
    match keychain {
        Some(keychain) => keychain
            .set_generic_password(service, account, secret)
            .map_err(|error| map_security_error(error, service, account)),
        None => SecKeychain::default()
            .map_err(|error| map_security_error(error, service, account))?
            .set_generic_password(service, account, secret)
            .map_err(|error| map_security_error(error, service, account)),
    }
}

fn get_generic_password_impl(
    keychain: Option<&SecKeychain>,
    service: &str,
    account: &str,
) -> Result<Vec<u8>> {
    let (password, _) = find_password_item(keychain, service, account)?;
    Ok(password.to_owned())
}

fn delete_generic_password_impl(
    keychain: Option<&SecKeychain>,
    service: &str,
    account: &str,
) -> Result<()> {
    let (_, item) = find_password_item(keychain, service, account)?;
    item.delete();
    Ok(())
}

pub(crate) fn save_generic_password(service: &str, account: &str, secret: &[u8]) -> Result<()> {
    save_generic_password_impl(None, service, account, secret)
}

pub(crate) fn get_generic_password(service: &str, account: &str) -> Result<Vec<u8>> {
    get_generic_password_impl(None, service, account)
}

pub(crate) fn delete_generic_password(service: &str, account: &str) -> Result<()> {
    delete_generic_password_impl(None, service, account)
}
