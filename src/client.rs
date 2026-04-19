use std::collections::HashSet;
use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::keychain::Result;
use crate::protocol::{SecretRequest, SecretResponse, WireResponse};

#[cfg(unix)]
pub(crate) fn load_namespace_env_from_socket(
    namespace: &str,
    socket_path: Option<&Path>,
) -> Result<Vec<(String, String)>> {
    let socket_path = resolve_socket_path(socket_path)?;
    fetch_namespace_env(namespace, &socket_path).map_err(Into::into)
}

#[cfg(not(unix))]
pub(crate) fn load_namespace_env_from_socket(
    _namespace: &str,
    _socket_path: Option<&Path>,
) -> Result<Vec<(String, String)>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "client-exec is only supported on unix platforms",
    )
    .into())
}

fn resolve_socket_path(socket_path: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(socket_path) = socket_path {
        return Ok(socket_path.to_path_buf());
    }

    match env::var_os("DIVECHAIN_SOCKET_PATH") {
        Some(socket_path) if !socket_path.is_empty() => Ok(PathBuf::from(socket_path)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "client-exec requires --socket-path or DIVECHAIN_SOCKET_PATH",
        )),
    }
}

#[cfg(unix)]
fn fetch_namespace_env(namespace: &str, socket_path: &Path) -> io::Result<Vec<(String, String)>> {
    let mut stream = UnixStream::connect(socket_path)?;
    let request = SecretRequest {
        namespace: namespace.to_owned(),
    };

    serde_json::to_writer(&mut stream, &request)?;
    stream.shutdown(Shutdown::Write)?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    parse_response(&response)
}

fn parse_response(response: &[u8]) -> io::Result<Vec<(String, String)>> {
    let response: WireResponse = serde_json::from_slice(response).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid server response: {}", error),
        )
    })?;

    match response {
        WireResponse::Success(response) => flatten_secrets(response),
        WireResponse::Error(response) => Err(io::Error::other(response.error.message)),
    }
}

fn flatten_secrets(response: SecretResponse) -> io::Result<Vec<(String, String)>> {
    if response.secrets.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid server response: secrets must not be empty",
        ));
    }

    let mut seen = HashSet::new();
    let mut secrets = Vec::with_capacity(response.secrets.len());

    for secret_entry in response.secrets {
        if secret_entry.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid server response: each secret entry must contain exactly one env var",
            ));
        }

        let (env, secret) = secret_entry
            .into_iter()
            .next()
            .expect("secret entries with len == 1 must contain a value");

        if !seen.insert(env.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid server response: duplicate env '{}'", env),
            ));
        }

        secrets.push((env, secret));
    }

    Ok(secrets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ErrorBody, ErrorResponse, WireResponse};
    use std::collections::HashMap;

    #[test]
    fn socket_path_argument_wins_over_environment() {
        let original = env::var_os("DIVECHAIN_SOCKET_PATH");
        unsafe { env::set_var("DIVECHAIN_SOCKET_PATH", "/tmp/from-env.sock") };

        let resolved =
            resolve_socket_path(Some(Path::new("/tmp/from-arg.sock"))).expect("socket path");

        match original {
            Some(value) => unsafe { env::set_var("DIVECHAIN_SOCKET_PATH", value) },
            None => unsafe { env::remove_var("DIVECHAIN_SOCKET_PATH") },
        }

        assert_eq!(resolved, PathBuf::from("/tmp/from-arg.sock"));
    }

    #[test]
    fn socket_path_falls_back_to_environment() {
        let original = env::var_os("DIVECHAIN_SOCKET_PATH");
        unsafe { env::set_var("DIVECHAIN_SOCKET_PATH", "/tmp/from-env.sock") };

        let resolved = resolve_socket_path(None).expect("socket path");

        match original {
            Some(value) => unsafe { env::set_var("DIVECHAIN_SOCKET_PATH", value) },
            None => unsafe { env::remove_var("DIVECHAIN_SOCKET_PATH") },
        }

        assert_eq!(resolved, PathBuf::from("/tmp/from-env.sock"));
    }

    #[test]
    fn socket_path_requires_argument_or_environment() {
        let original = env::var_os("DIVECHAIN_SOCKET_PATH");
        unsafe { env::remove_var("DIVECHAIN_SOCKET_PATH") };

        let error = resolve_socket_path(None).expect_err("socket path should be required");

        match original {
            Some(value) => unsafe { env::set_var("DIVECHAIN_SOCKET_PATH", value) },
            None => unsafe { env::remove_var("DIVECHAIN_SOCKET_PATH") },
        }

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "client-exec requires --socket-path or DIVECHAIN_SOCKET_PATH"
        );
    }

    #[test]
    fn parse_response_returns_flattened_secrets() {
        let response = serde_json::to_vec(&WireResponse::Success(SecretResponse {
            secrets: vec![
                HashMap::from([("AWS_ACCESS_KEY_ID".to_owned(), "key".to_owned())]),
                HashMap::from([("AWS_SECRET_ACCESS_KEY".to_owned(), "secret".to_owned())]),
            ],
        }))
        .expect("response should serialize");

        let secrets = parse_response(&response).expect("response should parse");

        assert_eq!(
            secrets,
            vec![
                ("AWS_ACCESS_KEY_ID".to_owned(), "key".to_owned()),
                ("AWS_SECRET_ACCESS_KEY".to_owned(), "secret".to_owned()),
            ]
        );
    }

    #[test]
    fn parse_response_returns_server_error_message() {
        let response = serde_json::to_vec(&WireResponse::Error(ErrorResponse {
            error: ErrorBody {
                code: "namespace_not_found".to_owned(),
                message: "namespace 'aws' does not exist".to_owned(),
            },
        }))
        .expect("response should serialize");

        let error = parse_response(&response).expect_err("server error should fail");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "namespace 'aws' does not exist");
    }

    #[test]
    fn parse_response_rejects_malformed_payload() {
        let error =
            parse_response(br#"{"secrets":"broken"}"#).expect_err("malformed response should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().starts_with("invalid server response:"));
    }

    #[test]
    fn parse_response_rejects_empty_secrets() {
        let response =
            serde_json::to_vec(&WireResponse::Success(SecretResponse { secrets: vec![] }))
                .expect("response should serialize");

        let error = parse_response(&response).expect_err("empty secrets should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "invalid server response: secrets must not be empty"
        );
    }

    #[test]
    fn parse_response_rejects_entries_with_multiple_keys() {
        let response = serde_json::to_vec(&WireResponse::Success(SecretResponse {
            secrets: vec![HashMap::from([
                ("AWS_ACCESS_KEY_ID".to_owned(), "key".to_owned()),
                ("AWS_SECRET_ACCESS_KEY".to_owned(), "secret".to_owned()),
            ])],
        }))
        .expect("response should serialize");

        let error = parse_response(&response).expect_err("multi-key entry should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "invalid server response: each secret entry must contain exactly one env var"
        );
    }

    #[test]
    fn parse_response_rejects_duplicate_env_names() {
        let response = serde_json::to_vec(&WireResponse::Success(SecretResponse {
            secrets: vec![
                HashMap::from([("AWS_ACCESS_KEY_ID".to_owned(), "key".to_owned())]),
                HashMap::from([("AWS_ACCESS_KEY_ID".to_owned(), "duplicate".to_owned())]),
            ],
        }))
        .expect("response should serialize");

        let error = parse_response(&response).expect_err("duplicate env should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "invalid server response: duplicate env 'AWS_ACCESS_KEY_ID'"
        );
    }
}
