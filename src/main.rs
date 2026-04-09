use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Read, Write};
use std::process;

use divechain::{KeychainStore, Result};

const USAGE: &str = "\
Usage:
  divechain --set <namespace> <ENV>
";

fn main() {
    if let Err(error) = run(env::args_os()) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(args: impl IntoIterator<Item = OsString>) -> Result<()> {
    let mut args = args.into_iter();
    let _binary = args.next();

    let Some(option) = args.next() else {
        print!("{USAGE}");
        return Ok(());
    };

    let store = KeychainStore::new();

    match option.to_string_lossy().as_ref() {
        "--set" => {
            let (namespace, env_name) = take_two(args)?;
            let secret = read_secret(&namespace, &env_name)?;
            store.save_generic_password(&namespace, &env_name, secret.as_bytes())
        }
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            process::exit(2);
        }
    }
}

fn take_two(mut args: impl Iterator<Item = OsString>) -> Result<(String, String)> {
    let namespace = next_required(&mut args, "<namespace>")?;
    let env_name = next_required(&mut args, "<ENV>")?;
    reject_extra(args)?;
    Ok((namespace, env_name))
}

fn next_required(args: &mut impl Iterator<Item = OsString>, name: &'static str) -> Result<String> {
    match args.next() {
        Some(value) => Ok(value.to_string_lossy().into_owned()),
        None => Err(usage_error(name)),
    }
}

fn reject_extra(mut args: impl Iterator<Item = OsString>) -> Result<()> {
    if args.next().is_some() {
        return Err(usage_error("too many arguments"));
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

fn usage_error(detail: &'static str) -> divechain::KeychainError {
    divechain::KeychainError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{detail}\n\n{USAGE}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::trim_trailing_newline;

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
