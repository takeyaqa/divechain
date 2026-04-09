use std::io::{self, IsTerminal, Read, Write};
use std::process;

use clap::{ArgAction, CommandFactory, Parser, error::ErrorKind};
use divechain::{KeychainStore, Result};

#[derive(Debug, Parser)]
#[command(
    arg_required_else_help = true,
    override_usage = "divechain -s|--set <namespace> <ENV>"
)]
struct Cli {
    #[arg(short = 's', long, action = ArgAction::SetTrue)]
    set: bool,
    #[arg(value_name = "namespace", requires = "set")]
    namespace: Option<String>,
    #[arg(value_name = "ENV", requires = "set")]
    env_name: Option<String>,
}

impl Cli {
    fn parse_from_args<I, T>(args: I) -> clap::error::Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let cli = Self::try_parse_from(args)?;
        cli.validate()
    }

    fn validate(self) -> clap::error::Result<Self> {
        if self.set && (self.namespace.is_none() || self.env_name.is_none()) {
            let mut command = Self::command();
            return Err(command.error(
                ErrorKind::MissingRequiredArgument,
                "the following required arguments were not provided:\n  <namespace>\n  <ENV>",
            ));
        }

        Ok(self)
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

    if cli.set {
        let namespace = cli.namespace.as_deref().expect("validated by clap");
        let env_name = cli.env_name.as_deref().expect("validated by clap");
        let secret = read_secret(namespace, env_name)?;
        return store.save_generic_password(namespace, env_name, secret.as_bytes());
    }

    Ok(())
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

    use super::{Cli, trim_trailing_newline};

    #[test]
    fn parses_set_invocation() {
        let cli = Cli::parse_from_args(["divechain", "--set", "aws", "AWS_ACCESS_KEY_ID"])
            .expect("set invocation should parse");

        assert!(cli.set);
        assert_eq!(cli.namespace.as_deref(), Some("aws"));
        assert_eq!(cli.env_name.as_deref(), Some("AWS_ACCESS_KEY_ID"));
    }

    #[test]
    fn parses_short_set_invocation() {
        let cli = Cli::parse_from_args(["divechain", "-s", "aws", "AWS_ACCESS_KEY_ID"])
            .expect("short set invocation should parse");

        assert!(cli.set);
        assert_eq!(cli.namespace.as_deref(), Some("aws"));
        assert_eq!(cli.env_name.as_deref(), Some("AWS_ACCESS_KEY_ID"));
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
        let error = Cli::parse_from_args(["divechain", "--set", "aws"])
            .expect_err("missing ENV should not parse");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rejects_positional_args_without_set() {
        let error = Cli::parse_from_args(["divechain", "aws", "AWS_ACCESS_KEY_ID"])
            .expect_err("positional args without --set should not parse");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn help_includes_set_usage() {
        let help = Cli::command().render_help().to_string();

        assert!(help.contains("divechain -s|--set <namespace> <ENV>"));
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
