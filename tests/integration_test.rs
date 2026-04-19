use snapbox::cmd::{Command, cargo_bin};

#[test]
fn set_and_unset_command_parses_namespace_and_env_name() {
    Command::new(cargo_bin("divechain"))
        .args(["set", "aws", "AWS_ENV"])
        .assert()
        .success();
    Command::new(cargo_bin("divechain"))
        .args(["unset", "aws", "AWS_ENV"])
        .assert()
        .success();
}

#[test]
fn list_command_prints_namespaces() {
    Command::new(cargo_bin("divechain"))
        .args(["set", "rails", "RAILS_ENV"])
        .assert()
        .success();

    Command::new(cargo_bin("divechain"))
        .args(["set", "github", "GITHUB_ENV"])
        .assert()
        .success();

    Command::new(cargo_bin("divechain"))
        .arg("list")
        .assert()
        .stdout_eq("github\nrails\n")
        .success();

    Command::new(cargo_bin("divechain"))
        .args(["unset", "rails", "RAILS_ENV"])
        .assert()
        .success();

    Command::new(cargo_bin("divechain"))
        .args(["unset", "github", "GITHUB_ENV"])
        .assert()
        .success();
}

#[test]
fn exec_command_parses_namespace_and_command() {
    Command::new(cargo_bin("divechain"))
        .args(["set", "codex", "CODEX_ENV"])
        .assert()
        .success();

    Command::new(cargo_bin("divechain"))
        .args(["exec", "codex", "--", "printenv", "CODEX_ENV"])
        .assert()
        .success();

    Command::new(cargo_bin("divechain"))
        .args(["unset", "codex", "CODEX_ENV"])
        .assert()
        .success();
}

#[test]
fn help_lists_supported_subcommands() {
    Command::new(cargo_bin("divechain"))
        .arg("help")
        .assert()
        .stdout_eq(
            r"A CLI for running commands with secrets from the macOS Keychain injected as environment variables.

Usage: divechain <COMMAND>

Commands:
  set     Set a secret for a namespace and environment variable name
  list    List all namespaces
  unset   Unset a secret for a namespace and environment variable name
  exec    Execute a command with environment variables from a namespace
  server  Start a secret server over a Unix domain socket
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
",
        )
        .success();
}

#[test]
fn rejects_empty_invocation() {
    Command::new(cargo_bin("divechain")).assert().failure();
}

#[test]
fn rejects_unknown_command() {
    Command::new(cargo_bin("divechain"))
        .arg("unknown")
        .assert()
        .failure();
}

#[test]
fn rejects_missing_env_for_set() {
    Command::new(cargo_bin("divechain"))
        .args(["set", "aws"])
        .assert()
        .failure();
}

#[test]
fn rejects_missing_env_for_unset() {
    Command::new(cargo_bin("divechain"))
        .args(["unset", "aws"])
        .assert()
        .failure();
}

#[test]
fn rejects_missing_command_for_exec() {
    Command::new(cargo_bin("divechain"))
        .args(["exec", "aws"])
        .assert()
        .failure();
}

#[test]
fn rejects_missing_socket_path_for_server() {
    Command::new(cargo_bin("divechain"))
        .arg("server")
        .assert()
        .failure();
}
