use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::protocol::{ErrorBody, ErrorResponse, SecretRequest, SecretResponse, WireResponse};
use crate::secret_store::{Result, SecretStore, SecretStoreError};

pub(crate) trait NamespaceSecretLoader {
    fn load_namespace_env(&self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>>;
}

impl NamespaceSecretLoader for SecretStore {
    fn load_namespace_env(&self, namespace: &str) -> Result<Vec<(String, Vec<u8>)>> {
        (*self).load_namespace_env(namespace)
    }
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
    match fs::symlink_metadata(socket_path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();

            if file_type.is_symlink() || !file_type.is_socket() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("socket path '{}' already exists", socket_path.display()),
                ));
            }

            match UnixStream::connect(socket_path) {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("socket path '{}' is already in use", socket_path.display()),
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(socket_path)?;
                }
                Err(error) => return Err(error),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
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
    serde_json::to_writer(&mut *stream, &response)?;
    stream.flush()
}

fn process_request<L: NamespaceSecretLoader>(loader: &L, request_bytes: &[u8]) -> WireResponse {
    match parse_request(request_bytes).and_then(|request| {
        eprintln!("{}", request_log_message(&request));
        load_response(loader, request)
    }) {
        Ok(response) => WireResponse::Success(response),
        Err(error) => WireResponse::Error(error.into_response()),
    }
}

fn parse_request(request_bytes: &[u8]) -> std::result::Result<SecretRequest, ServerError> {
    serde_json::from_slice(request_bytes)
        .map_err(|error| ServerError::InvalidRequest(format!("invalid request payload: {}", error)))
}

fn request_log_message(request: &SecretRequest) -> String {
    format!("received request for namespace '{}'", request.namespace)
}

fn load_response<L: NamespaceSecretLoader>(
    loader: &L,
    request: SecretRequest,
) -> std::result::Result<SecretResponse, ServerError> {
    let secrets = loader
        .load_namespace_env(&request.namespace)
        .map_err(|error| map_secret_store_error(error, &request.namespace))?;

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
) -> std::result::Result<std::collections::HashMap<String, String>, ServerError> {
    let secret = String::from_utf8(secret).map_err(|_| ServerError::InvalidSecretEncoding {
        namespace: namespace.to_owned(),
        env: env.clone(),
    })?;

    Ok(std::collections::HashMap::from([(env, secret)]))
}

fn map_secret_store_error(error: SecretStoreError, namespace: &str) -> ServerError {
    match error {
        SecretStoreError::NamespaceNotFound { .. } => {
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
                    code: "invalid_request".to_owned(),
                    message,
                },
            },
            Self::NamespaceNotFound(namespace) => ErrorResponse {
                error: ErrorBody {
                    code: "namespace_not_found".to_owned(),
                    message: format!("namespace '{}' does not exist", namespace),
                },
            },
            Self::InvalidSecretEncoding { namespace, env } => ErrorResponse {
                error: ErrorBody {
                    code: "invalid_secret_encoding".to_owned(),
                    message: format!("secret '{}.{}' is not valid UTF-8", namespace, env),
                },
            },
            Self::Internal(message) => ErrorResponse {
                error: ErrorBody {
                    code: "internal_error".to_owned(),
                    message,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[cfg(unix)]
    use std::net::Shutdown;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
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
                Err(SecretStoreError::NamespaceNotFound { namespace }) => {
                    Err(SecretStoreError::NamespaceNotFound {
                        namespace: namespace.clone(),
                    })
                }
                Err(SecretStoreError::SecretNotFound { namespace, env }) => {
                    Err(SecretStoreError::SecretNotFound {
                        namespace: namespace.clone(),
                        env: env.clone(),
                    })
                }
                Err(SecretStoreError::BackendFailure { code, message }) => {
                    Err(SecretStoreError::BackendFailure {
                        code: *code,
                        message: message.clone(),
                    })
                }
                Err(SecretStoreError::UnsupportedPlatform(platform)) => {
                    Err(SecretStoreError::UnsupportedPlatform(platform))
                }
                Err(SecretStoreError::Io(error)) => Err(SecretStoreError::Io(io::Error::new(
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
            WireResponse::Success(SecretResponse {
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
            WireResponse::Error(ErrorResponse {
                error: ErrorBody {
                    code: "namespace_not_found".to_owned(),
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
            WireResponse::Error(ErrorResponse {
                error: ErrorBody {
                    code,
                    ..
                },
            }) if code == "invalid_request"
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
            WireResponse::Error(ErrorResponse {
                error: ErrorBody {
                    code: "invalid_secret_encoding".to_owned(),
                    message: "secret 'github.ENV_NAME' is not valid UTF-8".to_owned(),
                },
            })
        );
    }

    #[test]
    fn request_log_message_includes_namespace() {
        let request = SecretRequest {
            namespace: "github".to_owned(),
        };

        assert_eq!(
            request_log_message(&request),
            "received request for namespace 'github'"
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
    fn bind_listener_rejects_existing_non_socket_path() {
        let socket_path = unique_socket_path();
        fs::write(&socket_path, []).expect("placeholder should be created");

        let error = bind_listener(&socket_path).expect_err("existing path should fail");
        fs::remove_file(&socket_path).expect("placeholder should be removed");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }

    #[cfg(unix)]
    #[test]
    fn bind_listener_reuses_stale_socket_path() {
        let socket_path = unique_socket_path();
        let listener = bind_listener(&socket_path).expect("listener should bind");

        drop(listener);

        let rebound = bind_listener(&socket_path).expect("stale socket path should rebind");
        drop(rebound);
        fs::remove_file(&socket_path).expect("socket file should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn bind_listener_rejects_active_socket_path() {
        let socket_path = unique_socket_path();
        let listener = bind_listener(&socket_path).expect("listener should bind");

        let error = bind_listener(&socket_path).expect_err("active socket path should fail");
        let client = UnixStream::connect(&socket_path).expect("active socket should remain usable");

        drop(client);
        drop(listener);
        fs::remove_file(&socket_path).expect("socket file should be removed");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }

    #[cfg(unix)]
    #[test]
    fn bind_listener_rejects_symlink_socket_path() {
        let target_path = unique_socket_path();
        let symlink_path = unique_socket_path();
        fs::write(&target_path, []).expect("target placeholder should be created");
        symlink(&target_path, &symlink_path).expect("symlink should be created");

        let error = bind_listener(&symlink_path).expect_err("symlink path should fail");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(symlink_path.exists(), "symlink should not be removed");

        fs::remove_file(&symlink_path).expect("symlink should be removed");
        fs::remove_file(&target_path).expect("target placeholder should be removed");
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
