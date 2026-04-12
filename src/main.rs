use std::io::{self, IsTerminal, Read};
use std::process;

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
    }
}

fn read_secret(namespace: &str, env_name: &str) -> Result<String> {
    if io::stdin().is_terminal() {
        let prompt = format!("{namespace}.{env_name}: ");
        rpassword::prompt_password(prompt)
            .map(normalize_secret)
            .map_err(Into::into)
    } else {
        let mut secret = String::new();
        io::stdin().read_to_string(&mut secret)?;
        Ok(normalize_secret(secret))
    }
}

fn normalize_secret(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser, error::ErrorKind};

    use super::{Cli, Commands, normalize_secret};

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
    }

    #[test]
    fn trims_single_newline() {
        assert_eq!(normalize_secret("secret\n".to_owned()), "secret");
    }

    #[test]
    fn trims_crlf() {
        assert_eq!(normalize_secret("secret\r\n".to_owned()), "secret");
    }

    #[test]
    fn trims_surrounding_spaces() {
        assert_eq!(normalize_secret("  secret  ".to_owned()), "secret");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(normalize_secret("\n\t secret \r\n".to_owned()), "secret");
    }

    #[test]
    fn preserves_internal_whitespace() {
        assert_eq!(
            normalize_secret("line 1\nline 2\n".to_owned()),
            "line 1\nline 2"
        );
    }
}
