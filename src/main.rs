use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process;

use divechain::{KeychainStore, Result};

const USAGE: &str = "\
Usage:
  divechain set <service> <account> <secret>
  divechain get <service> <account>
  divechain delete <service> <account>
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

    let Some(command) = args.next() else {
        print!("{USAGE}");
        return Ok(());
    };

    let store = KeychainStore::new();

    match command.to_string_lossy().as_ref() {
        "set" => {
            let (service, account, secret) = take_three(args)?;
            store.save_generic_password(&service, &account, secret.as_bytes())
        }
        "get" => {
            let (service, account) = take_two(args)?;
            let secret = store.get_generic_password(&service, &account)?;
            io::stdout().write_all(&secret)?;
            Ok(())
        }
        "delete" => {
            let (service, account) = take_two(args)?;
            store.delete_generic_password(&service, &account)
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
    let service = next_required(&mut args, "<service>")?;
    let account = next_required(&mut args, "<account>")?;
    reject_extra(args)?;
    Ok((service, account))
}

fn take_three(mut args: impl Iterator<Item = OsString>) -> Result<(String, String, String)> {
    let service = next_required(&mut args, "<service>")?;
    let account = next_required(&mut args, "<account>")?;
    let secret = next_required(&mut args, "<secret>")?;
    reject_extra(args)?;
    Ok((service, account, secret))
}

fn next_required(
    args: &mut impl Iterator<Item = OsString>,
    name: &'static str,
) -> Result<String> {
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

fn usage_error(detail: &'static str) -> divechain::KeychainError {
    divechain::KeychainError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{detail}\n\n{USAGE}"),
    ))
}
