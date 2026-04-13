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
            trailing_var_arg = true
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
    if io::stdin().is_terminal() {
        let prompt = format!("{namespace}.{env_name}: ");
        rpassword::prompt_password(prompt)
            .map(|value| value.trim().to_string())
            .map_err(Into::into)
    } else {
        let mut secret = String::new();
        io::stdin().read_to_string(&mut secret)?;
        Ok(secret.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::{CommandFactory, Parser, error::ErrorKind};

    use super::{Cli, Commands};

    #[test]
    fn parses_list_invocation() {
        let cli = Cli::try_parse_from(["divechain", "list"]).expect("list invocation should parse");

        match cli.command {
            Commands::List => {}
            other => panic!("expected list command, got {other:?}"),
        }
    }

    #[test]
    fn parses_set_invocation() {
        let cli = Cli::try_parse_from(["divechain", "set", "aws", "AWS_ACCESS_KEY_ID"])
            .expect("set invocation should parse");

        match cli.command {
            Commands::List => panic!("expected set command, got list"),
            Commands::Set {
                namespace,
                env_name,
            } => {
                assert_eq!(namespace, "aws");
                assert_eq!(env_name, "AWS_ACCESS_KEY_ID");
            }
            other => panic!("expected set command, got {other:?}"),
        }
    }

    #[test]
    fn parses_exec_invocation() {
        let cli =
            Cli::try_parse_from(["divechain", "exec", "aws", "printenv", "AWS_ACCESS_KEY_ID"])
                .expect("exec invocation should parse");

        match cli.command {
            Commands::Exec { namespace, command } => {
                assert_eq!(namespace, "aws");
                assert_eq!(
                    command,
                    vec![
                        OsString::from("printenv"),
                        OsString::from("AWS_ACCESS_KEY_ID"),
                    ]
                );
            }
            other => panic!("expected exec command, got {other:?}"),
        }
    }

    #[test]
    fn parses_exec_invocation_with_hyphenated_arguments() {
        let cli = Cli::try_parse_from([
            "divechain",
            "exec",
            "aws",
            "env",
            "-i",
            "FOO=bar",
            "printenv",
            "FOO",
        ])
        .expect("exec invocation with hyphenated arguments should parse");

        match cli.command {
            Commands::Exec { namespace, command } => {
                assert_eq!(namespace, "aws");
                assert_eq!(
                    command,
                    vec![
                        OsString::from("env"),
                        OsString::from("-i"),
                        OsString::from("FOO=bar"),
                        OsString::from("printenv"),
                        OsString::from("FOO"),
                    ]
                );
            }
            other => panic!("expected exec command, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_invocation() {
        let error =
            Cli::try_parse_from(["divechain"]).expect_err("empty invocation should not parse");

        assert_eq!(
            error.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn rejects_missing_env_for_set() {
        let error = Cli::try_parse_from(["divechain", "set", "aws"])
            .expect_err("missing env should not parse");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rejects_missing_command_for_exec() {
        let error = Cli::try_parse_from(["divechain", "exec", "aws"])
            .expect_err("missing command should not parse");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rejects_positional_args_without_set() {
        let error = Cli::try_parse_from(["divechain", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("positional args without set should not parse");

        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn rejects_long_set_flag() {
        let error = Cli::try_parse_from(["divechain", "--set", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("legacy --set should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn rejects_short_set_flag() {
        let error = Cli::try_parse_from(["divechain", "-s", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("legacy -s should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn help_includes_set_command() {
        let help = Cli::command().render_help().to_string();

        assert!(help.contains("Usage:"));
        assert!(help.contains("list"));
        assert!(help.contains("set"));
        assert!(help.contains("exec"));
    }
}
