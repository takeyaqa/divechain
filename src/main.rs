use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
#[cfg(unix)]
use std::os::unix::{ffi::OsStringExt, process::CommandExt};
use std::process::{self, Command};

use clap::{Parser, Subcommand};
use divechain::{KeychainStore, Result};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    List,
    Set {
        #[arg(value_name = "namespace")]
        namespace: String,
        #[arg(value_name = "env")]
        env_name: String,
    },
    Exec {
        #[arg(value_name = "namespace")]
        namespace: String,
        #[arg(
            value_name = "command",
            required = true,
            num_args = 1..,
            last = true
        )]
        command: Vec<OsString>,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let store = KeychainStore::new();

    match cli.command {
        Commands::List => {
            for namespace in store.list_namespaces()? {
                println!("{namespace}");
            }

            Ok(())
        }
        Commands::Set {
            namespace,
            env_name,
        } => {
            let secret = read_secret(&namespace, &env_name)?;
            store.save_generic_password(&namespace, &env_name, secret.as_bytes())
        }
        Commands::Exec { namespace, command } => exec_command(store, &namespace, command),
    }
}

#[cfg(unix)]
fn exec_command(store: KeychainStore, namespace: &str, command: Vec<OsString>) -> Result<()> {
    let mut command = command.into_iter();
    let program = command.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "exec requires a command to run",
        )
    })?;

    let mut process = Command::new(&program);
    process.args(command);

    for (env_name, secret) in store.load_namespace_env(namespace)? {
        process.env(env_name, OsString::from_vec(secret));
    }

    Err(process.exec().into())
}

#[cfg(not(unix))]
fn exec_command(_store: KeychainStore, _namespace: &str, _command: Vec<OsString>) -> Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exec is only supported on unix platforms",
    )
    .into())
}

fn read_secret(namespace: &str, env_name: &str) -> Result<String> {
    let secret = if io::stdin().is_terminal() {
        rpassword::prompt_password(format!("{}.{}: ", namespace, env_name))?
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
}
