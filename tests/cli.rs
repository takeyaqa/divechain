use snapbox::cmd::{Command, cargo_bin};

#[test]
#[ignore = "Requires user input, not suitable for automated testing"]
fn set_command_parses_namespace_and_env_name() {
    Command::new(cargo_bin("divechain"))
        .args(["set", "aws", "AWS_ACCESS_KEY_ID"])
        .assert()
        .success();
}

#[test]
#[ignore = "Requires user input, not suitable for automated testing"]
fn list_command_prints_namespaces() {
    Command::new(cargo_bin("divechain"))
        .arg("list")
        .assert()
        .stdout_eq("")
        .success();
}

#[test]
#[ignore = "Requires user input, not suitable for automated testing"]
fn exec_command_parses_namespace_and_command() {
    Command::new(cargo_bin("divechain"))
        .args(["exec", "aws", "--", "printenv", "AWS_ACCESS_KEY_ID"])
        .assert()
        .success();
}

#[test]
fn help_lists_supported_subcommands() {
    Command::new(cargo_bin("divechain"))
        .arg("help")
        .assert()
        .stdout_eq(
            r"Usage: divechain <COMMAND>

Commands:
  set    Set a secret for a namespace and environment variable name
  list   List all namespaces
  unset  Unset a secret for a namespace and environment variable name
  exec   Execute a command with environment variables from a namespace
  help   Print this message or the help of the given subcommand(s)

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
