use std::collections::{BTreeSet, HashMap};
use std::io;

use crate::keychain::{KeychainError, Result};

#[cfg(target_os = "macos")]
use security_framework::base::Error as SecurityError;
#[cfg(target_os = "macos")]
use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
#[cfg(target_os = "macos")]
use security_framework::passwords::{
    PasswordOptions, delete_generic_password, get_generic_password, set_generic_password_options,
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

    let services: Vec<_> = results
        .iter()
        .map(|result| {
            result
                .simplify_dict()
                .and_then(|attributes| attributes.get("svce").cloned())
        })
        .collect::<Vec<Option<String>>>()
        .into_iter()
        .flatten()
        .collect();

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

    let envs = collect_envs(
        results
            .iter()
            .map(|result| attributes_from_search_result(result, &service))
            .collect::<Result<Vec<_>>>()?,
        &service,
    )?;

    envs.into_iter()
        .map(|env| {
            get_generic_password(&service, &env)
                .map(|secret| (env, secret))
                .map_err(map_security_error)
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn map_security_error(error: SecurityError) -> KeychainError {
    KeychainError::KeychainFailure {
        code: error.code(),
        message: error.message(),
    }
}

fn map_delete_secret_error(
    namespace: &str,
    env: &str,
    code: i32,
    message: Option<String>,
) -> KeychainError {
    if code == ERR_SEC_ITEM_NOT_FOUND {
        KeychainError::SecretNotFound {
            namespace: namespace.to_owned(),
            env: env.to_owned(),
        }
    } else {
        KeychainError::KeychainFailure { code, message }
    }
}

fn keychain_service_name(namespace: &str) -> String {
    format!("{}{}", KEYCHAIN_SERVICE_PREFIX, namespace)
}

fn invalid_keychain_data(message: impl Into<String>) -> KeychainError {
    io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}

fn namespace_from_service(service: &str) -> Option<&str> {
    service
        .strip_prefix(KEYCHAIN_SERVICE_PREFIX)
        .filter(|namespace| !namespace.is_empty())
}

fn collect_namespaces(services: Vec<String>) -> Vec<String> {
    let mut cloned = services.clone();
    cloned.sort_unstable();
    cloned.dedup();

    cloned
        .iter()
        .map(|s| namespace_from_service(s))
        .flatten()
        .map(str::to_owned)
        .collect()
}

#[cfg(target_os = "macos")]
fn attributes_from_search_result(
    result: &SearchResult,
    service: &str,
) -> Result<HashMap<String, String>> {
    result.simplify_dict().ok_or_else(|| {
        invalid_keychain_data(format!(
            "keychain search result for service '{service}' is missing attributes"
        ))
    })
}

fn env_from_attributes(attributes: &HashMap<String, String>, service: &str) -> Result<String> {
    match attributes.get("acct") {
        Some(env) if !env.is_empty() => Ok(env.clone()),
        Some(_) => Err(invalid_keychain_data(format!(
            "keychain item for service '{service}' has an empty env name"
        ))),
        None => Err(invalid_keychain_data(format!(
            "keychain item for service '{service}' is missing an env name"
        ))),
    }
}

fn collect_envs<I>(attribute_sets: I, service: &str) -> Result<Vec<String>>
where
    I: IntoIterator<Item = HashMap<String, String>>,
{
    let mut envs = BTreeSet::new();

    for attributes in attribute_sets {
        let env = env_from_attributes(&attributes, service)?;

        if !envs.insert(env.clone()) {
            return Err(invalid_keychain_data(format!(
                "duplicate env name '{env}' found for service '{service}'"
            )));
        }
    }

    Ok(envs.into_iter().collect())
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use std::collections::HashMap;

    use super::*;
    use crate::KeychainError;

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
    fn extracts_env_name_from_attributes() {
        let env = env_from_attributes(
            &HashMap::from([("acct".to_owned(), "AWS_ACCESS_KEY_ID".to_owned())]),
            "divechain-aws",
        )
        .expect("acct attribute should be converted into an env name");

        assert_eq!(env, "AWS_ACCESS_KEY_ID");
    }

    #[test]
    fn rejects_missing_env_name_in_attributes() {
        let error = env_from_attributes(&HashMap::new(), "divechain-aws")
            .expect_err("missing acct should be rejected");

        assert_eq!(
            error.to_string(),
            "keychain item for service 'divechain-aws' is missing an env name"
        );
    }

    #[test]
    fn rejects_duplicate_env_names() {
        let error = collect_envs(
            [
                HashMap::from([("acct".to_owned(), "AWS_ACCESS_KEY_ID".to_owned())]),
                HashMap::from([("acct".to_owned(), "AWS_ACCESS_KEY_ID".to_owned())]),
            ],
            "divechain-aws",
        )
        .expect_err("duplicate env names should be rejected");

        assert_eq!(
            error.to_string(),
            "duplicate env name 'AWS_ACCESS_KEY_ID' found for service 'divechain-aws'"
        );
    }

    #[test]
    fn converts_missing_secret_delete_into_domain_error() {
        let error =
            map_delete_secret_error("aws", "AWS_ACCESS_KEY_ID", ERR_SEC_ITEM_NOT_FOUND, None);

        match error {
            KeychainError::SecretNotFound { namespace, env } => {
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
            KeychainError::KeychainFailure { code, message } => {
                assert_eq!(code, -1);
                assert_eq!(message.as_deref(), Some("boom"));
            }
            _ => panic!("expected keychain failure variant"),
        }
    }
}
