use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

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
  set          Set a secret for a namespace and environment variable name
  list         List all namespaces
  unset        Unset a secret for a namespace and environment variable name
  exec         Execute a command with environment variables from a namespace
  client-exec  Execute a command with environment variables from a socket-based secret server
  server       Start a secret server over a Unix domain socket
  help         Print this message or the help of the given subcommand(s)

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
fn client_exec_reads_socket_path_from_argument() {
    let socket_path = unique_socket_path();
    let handle = spawn_single_response_server(
        socket_path.clone(),
        r#"{"secrets":[{"AWS_ACCESS_KEY_ID":"secret-value"}]}"#.to_owned(),
    );

    Command::new(cargo_bin("divechain"))
        .arg("client-exec")
        .arg("aws")
        .arg("--socket-path")
        .arg(socket_path.as_os_str())
        .arg("--")
        .arg("printenv")
        .arg("AWS_ACCESS_KEY_ID")
        .assert()
        .stdout_eq("secret-value\n")
        .success();

    handle.join().expect("server thread should finish");
    fs::remove_file(&socket_path).expect("socket file should be removed");
}

#[test]
fn client_exec_reads_socket_path_from_environment() {
    let socket_path = unique_socket_path();
    let handle = spawn_single_response_server(
        socket_path.clone(),
        r#"{"secrets":[{"AWS_ACCESS_KEY_ID":"secret-value"}]}"#.to_owned(),
    );

    Command::new(cargo_bin("divechain"))
        .arg("client-exec")
        .arg("aws")
        .arg("--")
        .arg("printenv")
        .arg("AWS_ACCESS_KEY_ID")
        .env("DIVECHAIN_SOCKET_PATH", socket_path.as_os_str())
        .assert()
        .stdout_eq("secret-value\n")
        .success();

    handle.join().expect("server thread should finish");
    fs::remove_file(&socket_path).expect("socket file should be removed");
}

#[test]
fn client_exec_argument_takes_priority_over_environment() {
    let socket_path = unique_socket_path();
    let handle = spawn_single_response_server(
        socket_path.clone(),
        r#"{"secrets":[{"AWS_ACCESS_KEY_ID":"secret-value"}]}"#.to_owned(),
    );

    Command::new(cargo_bin("divechain"))
        .arg("client-exec")
        .arg("aws")
        .arg("--socket-path")
        .arg(socket_path.as_os_str())
        .arg("--")
        .arg("printenv")
        .arg("AWS_ACCESS_KEY_ID")
        .env("DIVECHAIN_SOCKET_PATH", "/tmp/does-not-exist.sock")
        .assert()
        .stdout_eq("secret-value\n")
        .success();

    handle.join().expect("server thread should finish");
    fs::remove_file(&socket_path).expect("socket file should be removed");
}

#[test]
fn client_exec_stops_when_server_returns_error() {
    let socket_path = unique_socket_path();
    let marker_path = unique_marker_path();
    let command = format!("touch {}", marker_path.display());
    let handle = spawn_single_response_server(
        socket_path.clone(),
        r#"{"error":{"code":"namespace_not_found","message":"namespace 'aws' does not exist"}}"#
            .to_owned(),
    );

    Command::new(cargo_bin("divechain"))
        .arg("client-exec")
        .arg("aws")
        .arg("--socket-path")
        .arg(socket_path.as_os_str())
        .arg("--")
        .arg("/bin/sh")
        .arg("-c")
        .arg(&command)
        .assert()
        .stderr_eq("error: namespace 'aws' does not exist\n")
        .failure();

    handle.join().expect("server thread should finish");
    fs::remove_file(&socket_path).expect("socket file should be removed");
    assert!(
        !marker_path.exists(),
        "client-exec should not run the command on server errors"
    );
}

#[test]
fn rejects_missing_socket_path_for_server() {
    Command::new(cargo_bin("divechain"))
        .arg("server")
        .assert()
        .failure();
}

#[test]
fn rejects_missing_socket_path_for_client_exec() {
    Command::new(cargo_bin("divechain"))
        .args(["client-exec", "aws", "--", "printenv", "AWS_ACCESS_KEY_ID"])
        .assert()
        .failure();
}

fn spawn_single_response_server(socket_path: PathBuf, response: String) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = UnixListener::bind(&socket_path).expect("listener should bind");
        let (mut stream, _) = listener.accept().expect("listener should accept");

        let mut request = String::new();
        stream
            .read_to_string(&mut request)
            .expect("request should read");
        assert_eq!(request, r#"{"namespace":"aws"}"#);

        stream
            .write_all(response.as_bytes())
            .expect("response should write");
    })
}

fn unique_socket_path() -> PathBuf {
    unique_temp_path("sock")
}

fn unique_marker_path() -> PathBuf {
    unique_temp_path("marker")
}

fn unique_temp_path(extension: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be valid")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "divechain-client-exec-{}-{}.{}",
        std::process::id(),
        nonce,
        extension
    ))
}
