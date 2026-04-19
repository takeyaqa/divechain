use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::net::UnixListener;

use serde::{Deserialize, Serialize};

use crate::keychain::{KeychainError, KeychainStore, Result};

pub(crate) trait NamespaceSecretLoader {
    fn load_namespace_env(&self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>>;
}

impl NamespaceSecretLoader for KeychainStore {
    fn load_namespace_env(&self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        (*self).load_namespace_env(namespace)
    }
}

#[derive(Debug, Deserialize)]
struct SecretRequest {
    namespace: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct SecretResponse {
    secrets: Vec<HashMap<String, String>>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Response {
    Success(SecretResponse),
    Error(ErrorResponse),
}

#[derive(Debug, PartialEq, Eq)]
enum ServerError {
    InvalidRequest(String),
    NamespaceNotFound(String),
    InvalidSecretEncoding { namespace: String, env: String },
    Internal(String),
}

#[cfg(unix)]
pub(crate) fn run_server<L: NamespaceSecretLoader>(loader: L, socket_path: &Path) -> Result<()> {
    let listener = bind_listener(socket_path)?;
    eprintln!("listening on {}", socket_path.display());

    loop {
        let (mut stream, _) = listener.accept()?;

        if let Err(error) = handle_stream(&loader, &mut stream) {
            eprintln!("server connection error: {}", error);
        }
    }
}

#[cfg(not(unix))]
pub(crate) fn run_server<L: NamespaceSecretLoader>(_loader: L, _socket_path: &Path) -> Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "server is only supported on unix platforms",
    )
    .into())
}

#[cfg(unix)]
fn bind_listener(socket_path: &Path) -> io::Result<UnixListener> {
    if socket_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("socket path '{}' already exists", socket_path.display()),
        ));
    }

    UnixListener::bind(socket_path)
}

fn handle_stream<L, S>(loader: &L, stream: &mut S) -> io::Result<()>
where
    L: NamespaceSecretLoader,
    S: Read + Write,
{
    let mut request_bytes = Vec::new();
    stream.read_to_end(&mut request_bytes)?;

    let response = process_request(loader, &request_bytes);
    serde_json::to_writer(&mut *stream, &response.into_wire())?;
    stream.flush()
}

fn process_request<L: NamespaceSecretLoader>(loader: &L, request_bytes: &[u8]) -> Response {
    match parse_request(request_bytes).and_then(|request| load_response(loader, request)) {
        Ok(response) => Response::Success(response),
        Err(error) => Response::Error(error.into_response()),
    }
}

fn parse_request(request_bytes: &[u8]) -> std::result::Result<SecretRequest, ServerError> {
    serde_json::from_slice(request_bytes)
        .map_err(|error| ServerError::InvalidRequest(format!("invalid request payload: {}", error)))
}

fn load_response<L: NamespaceSecretLoader>(
    loader: &L,
    request: SecretRequest,
) -> std::result::Result<SecretResponse, ServerError> {
    let secrets = loader
        .load_namespace_env(&request.namespace)
        .map_err(|error| map_keychain_error(error, &request.namespace))?;

    if secrets.is_empty() {
        return Err(ServerError::NamespaceNotFound(request.namespace));
    }

    let secrets = secrets
        .into_iter()
        .map(|(env, secret)| decode_secret_entry(&request.namespace, env, secret))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(SecretResponse { secrets })
}

fn decode_secret_entry(
    namespace: &str,
    env: String,
    secret: Vec<u8>,
) -> std::result::Result<HashMap<String, String>, ServerError> {
    let secret = String::from_utf8(secret).map_err(|_| ServerError::InvalidSecretEncoding {
        namespace: namespace.to_owned(),
        env: env.clone(),
    })?;

    Ok(HashMap::from([(env, secret)]))
}

fn map_keychain_error(error: KeychainError, namespace: &str) -> ServerError {
    match error {
        KeychainError::NamespaceNotFound { .. } => {
            ServerError::NamespaceNotFound(namespace.to_owned())
        }
        other => ServerError::Internal(other.to_string()),
    }
}

impl ServerError {
    fn into_response(self) -> ErrorResponse {
        match self {
            Self::InvalidRequest(message) => ErrorResponse {
                error: ErrorBody {
                    code: "invalid_request",
                    message,
                },
            },
            Self::NamespaceNotFound(namespace) => ErrorResponse {
                error: ErrorBody {
                    code: "namespace_not_found",
                    message: format!("namespace '{}' does not exist", namespace),
                },
            },
            Self::InvalidSecretEncoding { namespace, env } => ErrorResponse {
                error: ErrorBody {
                    code: "invalid_secret_encoding",
                    message: format!("secret '{}.{}' is not valid UTF-8", namespace, env),
                },
            },
            Self::Internal(message) => ErrorResponse {
                error: ErrorBody {
                    code: "internal_error",
                    message,
                },
            },
        }
    }
}

impl Response {
    fn into_wire(self) -> impl Serialize {
        match self {
            Self::Success(response) => WireResponse::Success(response),
            Self::Error(response) => WireResponse::Error(response),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum WireResponse {
    Success(SecretResponse),
    Error(ErrorResponse),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::net::Shutdown;
    #[cfg(unix)]
    use std::os::unix::net::UnixStream;
    #[cfg(unix)]
    use std::thread;
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    struct StubLoader {
        result: Result<Vec<(String, Vec<u8>)>>,
    }

    impl NamespaceSecretLoader for StubLoader {
        fn load_namespace_env(&self, _namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
            match &self.result {
                Ok(secrets) => Ok(secrets.clone()),
                Err(KeychainError::NamespaceNotFound { namespace }) => {
                    Err(KeychainError::NamespaceNotFound {
                        namespace: namespace.clone(),
                    })
                }
                Err(KeychainError::SecretNotFound { namespace, env }) => {
                    Err(KeychainError::SecretNotFound {
                        namespace: namespace.clone(),
                        env: env.clone(),
                    })
                }
                Err(KeychainError::KeychainFailure { code, message }) => {
                    Err(KeychainError::KeychainFailure {
                        code: *code,
                        message: message.clone(),
                    })
                }
                Err(KeychainError::UnsupportedPlatform(platform)) => {
                    Err(KeychainError::UnsupportedPlatform(platform))
                }
                Err(KeychainError::Io(error)) => Err(KeychainError::Io(io::Error::new(
                    error.kind(),
                    error.to_string(),
                ))),
            }
        }
    }

    #[test]
    fn process_request_returns_secrets_array() {
        let loader = StubLoader {
            result: Ok(vec![
                ("ENV_NAME".to_owned(), b"secret-value".to_vec()),
                ("OTHER_ENV".to_owned(), b"other-secret".to_vec()),
            ]),
        };

        let response = process_request(&loader, br#"{"namespace":"github"}"#);

        assert_eq!(
            response,
            Response::Success(SecretResponse {
                secrets: vec![
                    HashMap::from([("ENV_NAME".to_owned(), "secret-value".to_owned())]),
                    HashMap::from([("OTHER_ENV".to_owned(), "other-secret".to_owned())]),
                ],
            })
        );
    }

    #[test]
    fn process_request_rejects_unknown_namespace() {
        let loader = StubLoader { result: Ok(vec![]) };

        let response = process_request(&loader, br#"{"namespace":"github"}"#);

        assert_eq!(
            response,
            Response::Error(ErrorResponse {
                error: ErrorBody {
                    code: "namespace_not_found",
                    message: "namespace 'github' does not exist".to_owned(),
                },
            })
        );
    }

    #[test]
    fn process_request_rejects_invalid_json() {
        let loader = StubLoader { result: Ok(vec![]) };

        let response = process_request(&loader, br#"{"namespace":"github""#);

        assert!(matches!(
            response,
            Response::Error(ErrorResponse {
                error: ErrorBody {
                    code: "invalid_request",
                    ..
                },
            })
        ));
    }

    #[test]
    fn process_request_rejects_non_utf8_secrets() {
        let loader = StubLoader {
            result: Ok(vec![("ENV_NAME".to_owned(), vec![0xff, 0xfe])]),
        };

        let response = process_request(&loader, br#"{"namespace":"github"}"#);

        assert_eq!(
            response,
            Response::Error(ErrorResponse {
                error: ErrorBody {
                    code: "invalid_secret_encoding",
                    message: "secret 'github.ENV_NAME' is not valid UTF-8".to_owned(),
                },
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn socket_round_trip_returns_single_response() {
        let socket_path = unique_socket_path();
        let listener = bind_listener(&socket_path).expect("listener should bind");
        let loader = StubLoader {
            result: Ok(vec![("ENV_NAME".to_owned(), b"secret-value".to_vec())]),
        };

        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("listener should accept");
            handle_stream(&loader, &mut stream).expect("handler should succeed");
        });

        let mut client = UnixStream::connect(&socket_path).expect("client should connect");
        client
            .write_all(br#"{"namespace":"github"}"#)
            .expect("request should write");
        client
            .shutdown(Shutdown::Write)
            .expect("write half should shutdown");

        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .expect("response should read");

        server_thread.join().expect("server thread should finish");
        fs::remove_file(&socket_path).expect("socket file should be removed");

        assert_eq!(response, r#"{"secrets":[{"ENV_NAME":"secret-value"}]}"#);
    }

    #[cfg(unix)]
    #[test]
    fn bind_listener_rejects_existing_socket_path() {
        let socket_path = unique_socket_path();
        fs::write(&socket_path, []).expect("placeholder should be created");

        let error = bind_listener(&socket_path).expect_err("existing path should fail");
        fs::remove_file(&socket_path).expect("placeholder should be removed");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }

    #[cfg(unix)]
    fn unique_socket_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be valid")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "divechain-server-test-{}-{}.sock",
            std::process::id(),
            nonce
        ))
    }
}
