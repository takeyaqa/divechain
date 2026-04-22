mod client;
mod macos_keychain;
mod protocol;
mod secret_store;
mod server;

use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
#[cfg(unix)]
use std::os::unix::{ffi::OsStringExt, process::CommandExt};
use std::path::PathBuf;
use std::process::{self, Command};

use crate::secret_store::{Result, SecretStore};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Set a secret for a namespace and environment variable name
    Set {
        /// The namespace to store the secret under
        #[arg(required = true)]
        namespace: String,
        /// The environment variable name to store the secret under
        #[arg(required = true)]
        env: String,
    },
    /// List all namespaces
    List,
    /// Unset a secret for a namespace and environment variable name
    Unset {
        /// The namespace to delete the secret from
        #[arg(required = true)]
        namespace: String,
        /// The environment variable name to delete the secret from
        #[arg(required = true)]
        env: String,
    },
    /// Execute a command with environment variables from a namespace
    Exec {
        /// The namespace to load environment variables from
        #[arg(required = true)]
        namespace: String,
        /// The command to execute with environment variables from the namespace
        #[arg(
            required = true,
            num_args = 1..,
            last = true
        )]
        command: Vec<OsString>,
    },
    /// Execute a command with environment variables from a socket-based secret server
    ClientExec {
        /// The namespace to load environment variables from
        #[arg(required = true)]
        namespace: String,
        /// The Unix domain socket path to connect to
        #[arg(long)]
        socket_path: Option<PathBuf>,
        /// The command to execute with environment variables from the namespace
        #[arg(
            required = true,
            num_args = 1..,
            last = true
        )]
        command: Vec<OsString>,
    },
    /// Start a secret server over a Unix domain socket
    Server {
        /// The Unix domain socket path to bind
        #[arg(long, required = true)]
        socket_path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli) {
        eprintln!("error: {}", error);
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let store = SecretStore::new();

    match cli.command {
        Commands::List => {
            for namespace in store.list_namespaces()? {
                println!("{}", namespace);
            }

            Ok(())
        }
        Commands::Set { namespace, env } => {
            let secret = read_secret(&namespace, &env)?;
            store.save_secret(&namespace, &env, secret.as_bytes())
        }
        Commands::Unset { namespace, env } => store.delete_secret(&namespace, &env),
        Commands::Exec { namespace, command } => exec_command(store, &namespace, command),
        Commands::ClientExec {
            namespace,
            socket_path,
            command,
        } => client_exec_command(&namespace, socket_path.as_deref(), command),
        Commands::Server { socket_path } => server::run_server(store, &socket_path),
    }
}

#[cfg(unix)]
fn exec_command(store: SecretStore, namespace: &str, command: Vec<OsString>) -> Result<()> {
    let envs = store
        .load_namespace_env(namespace)?
        .into_iter()
        .map(|(env, secret)| (env, OsString::from_vec(secret)));

    exec_with_env(command, envs)
}

#[cfg(unix)]
fn client_exec_command(
    namespace: &str,
    socket_path: Option<&std::path::Path>,
    command: Vec<OsString>,
) -> Result<()> {
    let envs = client::load_namespace_env_from_socket(namespace, socket_path)?
        .into_iter()
        .map(|(env, secret)| (env, OsString::from(secret)));

    exec_with_env(command, envs)
}

#[cfg(unix)]
fn exec_with_env<I>(command: Vec<OsString>, envs: I) -> Result<()>
where
    I: IntoIterator<Item = (String, OsString)>,
{
    let mut command = command.into_iter();
    let program = command.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "command execution requires a command to run",
        )
    })?;

    let mut process = Command::new(&program);
    process.args(command);

    for (env, secret) in envs {
        process.env(env, secret);
    }

    Err(process.exec().into())
}

#[cfg(not(unix))]
fn exec_command(_store: SecretStore, _namespace: &str, _command: Vec<OsString>) -> Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exec is only supported on unix platforms",
    )
    .into())
}

#[cfg(not(unix))]
fn client_exec_command(
    _namespace: &str,
    _socket_path: Option<&std::path::Path>,
    _command: Vec<OsString>,
) -> Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "client-exec is only supported on unix platforms",
    )
    .into())
}

fn read_secret(namespace: &str, env: &str) -> Result<String> {
    let secret = if io::stdin().is_terminal() {
        rpassword::prompt_password(format!("{}.{}: ", namespace, env))?
    } else {
        let mut secret = String::new();
        io::stdin().read_to_string(&mut secret)?;
        secret
    };
    Ok(secret.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn server_command_parses_socket_path() {
        let cli = Cli::try_parse_from([
            "divechain",
            "server",
            "--socket-path",
            "/tmp/divechain.sock",
        ])
        .expect("server command should parse");

        match cli.command {
            Commands::Server { socket_path } => {
                assert_eq!(socket_path, PathBuf::from("/tmp/divechain.sock"));
            }
            other => panic!("expected server command, got {:?}", other),
        }
    }

    #[test]
    fn client_exec_command_parses_socket_path() {
        let cli = Cli::try_parse_from([
            "divechain",
            "client-exec",
            "aws",
            "--socket-path",
            "/tmp/divechain.sock",
            "--",
            "env",
        ])
        .expect("client-exec command should parse");

        match cli.command {
            Commands::ClientExec {
                namespace,
                socket_path,
                command,
            } => {
                assert_eq!(namespace, "aws");
                assert_eq!(socket_path, Some(PathBuf::from("/tmp/divechain.sock")));
                assert_eq!(command, vec![OsString::from("env")]);
            }
            other => panic!("expected client-exec command, got {:?}", other),
        }
    }
}
