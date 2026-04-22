use crate::secret_store::{Result, SecretStoreError};

#[cfg(target_os = "macos")]
use security_framework::base::Error as SecurityError;
#[cfg(target_os = "macos")]
use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
#[cfg(target_os = "macos")]
use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password_options,
};

const KEYCHAIN_SERVICE_PREFIX: &str = "divechain-";
const KEYCHAIN_ITEM_LABEL: &str = "divechain";
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

#[cfg(target_os = "macos")]
pub(crate) fn save_secret(namespace: &str, env: &str, secret: &[u8]) -> Result<()> {
    let service = keychain_service_name(namespace);
    let mut options = PasswordOptions::new_generic_password(&service, env);
    options.set_label(KEYCHAIN_ITEM_LABEL);

    set_generic_password_options(secret, options).map_err(map_security_error)
}

#[cfg(target_os = "macos")]
pub(crate) fn delete_secret(namespace: &str, env: &str) -> Result<()> {
    let service = keychain_service_name(namespace);

    delete_generic_password(&service, env)
        .map_err(|error| map_delete_secret_error(namespace, env, error.code(), error.message()))
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
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(vec![]),
        Err(error) => return Err(map_security_error(error)),
    };

    let services: Vec<_> = extract_attribute(results, "svce");

    Ok(collect_namespaces(services))
}

#[cfg(target_os = "macos")]
pub(crate) fn load_namespace_env(namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let service = keychain_service_name(namespace);
    let mut options = ItemSearchOptions::new();
    options
        .class(ItemClass::generic_password())
        .service(&service)
        .label(KEYCHAIN_ITEM_LABEL)
        .load_attributes(true)
        .limit(Limit::All);

    let results = match options.search() {
        Ok(results) => results,
        Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(vec![]),
        Err(error) => return Err(map_security_error(error)),
    };

    let accounts: Vec<_> = extract_attribute(results, "acct");

    collect_secret(accounts, &service)
}

#[cfg(target_os = "macos")]
fn map_security_error(error: SecurityError) -> SecretStoreError {
    SecretStoreError::BackendFailure {
        code: error.code(),
        message: error.message(),
    }
}

fn map_delete_secret_error(
    namespace: &str,
    env: &str,
    code: i32,
    message: Option<String>,
) -> SecretStoreError {
    if code == ERR_SEC_ITEM_NOT_FOUND {
        SecretStoreError::SecretNotFound {
            namespace: namespace.to_owned(),
            env: env.to_owned(),
        }
    } else {
        SecretStoreError::BackendFailure { code, message }
    }
}

fn keychain_service_name(namespace: &str) -> String {
    format!("{}{}", KEYCHAIN_SERVICE_PREFIX, namespace)
}

fn namespace_from_service(service: &str) -> Option<&str> {
    service
        .strip_prefix(KEYCHAIN_SERVICE_PREFIX)
        .filter(|namespace| !namespace.is_empty())
}

#[cfg(target_os = "macos")]
fn extract_attribute(search_results: Vec<SearchResult>, attribute: &str) -> Vec<String> {
    search_results
        .iter()
        .filter_map(|result| {
            result
                .simplify_dict()
                .and_then(|attributes| attributes.get(attribute).cloned())
        })
        .collect()
}

fn collect_namespaces(mut services: Vec<String>) -> Vec<String> {
    services.sort_unstable();
    services.dedup();

    services
        .iter()
        .filter_map(|s| namespace_from_service(s))
        .map(str::to_owned)
        .collect()
}

#[cfg(target_os = "macos")]
fn collect_secret(accounts: Vec<String>, service: &str) -> Result<Vec<(String, Vec<u8>)>> {
    accounts
        .into_iter()
        .map(|account| {
            generic_password(PasswordOptions::new_generic_password(service, &account))
                .map(|secret| (account, secret))
                .map_err(map_security_error)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let namespaces = collect_namespaces(vec![
            "divechain-zsh".to_owned(),
            "divechain-aws".to_owned(),
            "divechain-github".to_owned(),
            "divechain-aws".to_owned(),
            "not-divechain-ignored".to_owned(),
            "divechain-".to_owned(),
        ]);

        assert_eq!(namespaces, vec!["aws", "github", "zsh"]);
    }

    #[test]
    fn converts_missing_secret_delete_into_domain_error() {
        let error =
            map_delete_secret_error("aws", "AWS_ACCESS_KEY_ID", ERR_SEC_ITEM_NOT_FOUND, None);

        match error {
            SecretStoreError::SecretNotFound { namespace, env } => {
                assert_eq!(namespace, "aws");
                assert_eq!(env, "AWS_ACCESS_KEY_ID");
            }
            _ => panic!("expected missing secret error variant"),
        }
    }

    #[test]
    fn preserves_non_missing_delete_errors() {
        let error =
            map_delete_secret_error("aws", "AWS_ACCESS_KEY_ID", -1, Some("boom".to_owned()));

        match error {
            SecretStoreError::BackendFailure { code, message } => {
                assert_eq!(code, -1);
                assert_eq!(message.as_deref(), Some("boom"));
            }
            _ => panic!("expected keychain failure variant"),
        }
    }
}
