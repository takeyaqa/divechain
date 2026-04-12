use std::collections::BTreeSet;

use crate::keychain::{KeychainError, Result};

#[cfg(target_os = "macos")]
use security_framework::base::Error as SecurityError;
#[cfg(target_os = "macos")]
use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
#[cfg(target_os = "macos")]
use security_framework::passwords::{PasswordOptions, set_generic_password_options};

const KEYCHAIN_SERVICE_PREFIX: &str = "divechain-";
const KEYCHAIN_ITEM_LABEL: &str = "divechain";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

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
fn is_item_not_found(code: i32) -> bool {
    code == ERR_SEC_ITEM_NOT_FOUND
}

fn namespace_from_service(service: &str) -> Option<&str> {
    service
        .strip_prefix(KEYCHAIN_SERVICE_PREFIX)
        .filter(|namespace| !namespace.is_empty())
}

fn collect_namespaces<I, S>(services: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    services
        .into_iter()
        .filter_map(|service| namespace_from_service(service.as_ref()).map(str::to_owned))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(target_os = "macos")]
fn service_from_search_result(result: &SearchResult) -> Option<String> {
    result
        .simplify_dict()
        .and_then(|attributes| attributes.get("svce").cloned())
}

#[cfg(target_os = "macos")]
pub(crate) fn list_namespaces() -> Result<Vec<String>> {
    let mut options = ItemSearchOptions::new();
    options
        .class(ItemClass::generic_password())
        .label(KEYCHAIN_ITEM_LABEL)
        .load_attributes(true)
        .limit(Limit::All);

    let results = match options.search() {
        Ok(results) => results,
        Err(error) if is_item_not_found(error.code()) => return Ok(vec![]),
        Err(error) => return Err(map_security_error(error)),
    };

    Ok(collect_namespaces(
        results.iter().filter_map(service_from_search_result),
    ))
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
    #[cfg(target_os = "macos")]
    use super::is_item_not_found;
    use super::{
        ERR_SEC_ITEM_NOT_FOUND, collect_namespaces, keychain_service_name, namespace_from_service,
    };

    #[test]
    fn prefixes_namespace_in_keychain_service_name() {
        assert_eq!(keychain_service_name("aws"), "divechain-aws");
    }

    #[test]
    fn extracts_namespace_from_service_name() {
        assert_eq!(namespace_from_service("divechain-aws"), Some("aws"));
    }

    #[test]
    fn ignores_malformed_service_names() {
        assert_eq!(namespace_from_service("divechain-"), None);
        assert_eq!(namespace_from_service("not-divechain-aws"), None);
    }

    #[test]
    fn collects_unique_namespaces_in_sorted_order() {
        let namespaces = collect_namespaces([
            "divechain-zsh",
            "divechain-aws",
            "divechain-github",
            "divechain-aws",
            "not-divechain-ignored",
            "divechain-",
        ]);

        assert_eq!(namespaces, vec!["aws", "github", "zsh"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn treats_item_not_found_as_empty_result() {
        assert!(is_item_not_found(ERR_SEC_ITEM_NOT_FOUND));
        assert!(!is_item_not_found(-1));
    }
}
