use std::io::{self, IsTerminal, Read, Write};
use std::process;

use clap::{Parser, Subcommand};
use divechain::{KeychainStore, Result};

#[derive(Debug, Parser)]
#[command(
    arg_required_else_help = true,
    override_usage = "divechain set <namespace> <env>"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Set {
        #[arg(value_name = "namespace")]
        namespace: String,
        #[arg(value_name = "env")]
        env_name: String,
    },
}

impl Cli {
    fn parse_from_args<I, T>(args: I) -> clap::error::Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
    }
}

fn main() {
    let cli = match Cli::parse_from_args(std::env::args_os()) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };

    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let store = KeychainStore::new();

    match cli.command {
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
        let mut stderr = io::stderr().lock();
        write!(stderr, "{namespace}.{env_name}: ")?;
        stderr.flush()?;

        return rpassword::read_password().map_err(Into::into);
    }

    let mut secret = String::new();
    io::stdin().read_to_string(&mut secret)?;
    Ok(trim_trailing_newline(secret))
}

fn trim_trailing_newline(mut value: String) -> String {
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, error::ErrorKind};

    use super::{Cli, Commands, trim_trailing_newline};

    #[test]
    fn parses_set_invocation() {
        let cli = Cli::parse_from_args(["divechain", "set", "aws", "AWS_ACCESS_KEY_ID"])
            .expect("set invocation should parse");

        match cli.command {
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
            Cli::parse_from_args(["divechain"]).expect_err("empty invocation should not parse");

        assert!(matches!(
            error.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                | ErrorKind::MissingRequiredArgument
        ));
    }

    #[test]
    fn rejects_missing_env_for_set() {
        let error = Cli::parse_from_args(["divechain", "set", "aws"])
            .expect_err("missing env should not parse");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rejects_positional_args_without_set() {
        let error = Cli::parse_from_args(["divechain", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("positional args without set should not parse");

        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn rejects_long_set_flag() {
        let error = Cli::parse_from_args(["divechain", "--set", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("legacy --set should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn rejects_short_set_flag() {
        let error = Cli::parse_from_args(["divechain", "-s", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("legacy -s should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn help_includes_set_usage() {
        let help = Cli::command().render_help().to_string();

        assert!(help.contains("divechain set <namespace> <env>"));
    }

    #[test]
    fn trims_single_newline() {
        assert_eq!(trim_trailing_newline("secret\n".to_owned()), "secret");
    }

    #[test]
    fn trims_crlf() {
        assert_eq!(trim_trailing_newline("secret\r\n".to_owned()), "secret");
    }

    #[test]
    fn preserves_internal_whitespace() {
        assert_eq!(
            trim_trailing_newline("line 1\nline 2\n".to_owned()),
            "line 1\nline 2"
        );
    }
}
