# divechain

`divechain` is a CLI for running commands with secrets from the macOS Keychain injected as environment variables.

## Usage

### `set <namespace> <env>`

```console
$ divechain set github GITHUB_TOKEN
github.GITHUB_TOKEN: 
```

`set` creates or updates a secrets in the default user keychain.
When `namespace` is `github`, the macOS Keychain service name is `divechain-github`.
Saved macOS Keychain items also use the fixed Keychain label `divechain`.
When running in a TTY, the secret is read interactively without echo.

You can also pipe the secret in non-interactive environments:

```console
$ ./fetch_secret_from_somewhere.sh | divechain set github GITHUB_TOKEN
```

### `list`

```console
$ divechain list
aws
github
```

`list` searches secrets whose Keychain label is `divechain`, extracts the namespace, removes duplicates, and prints namespaces only, one per line, in alphabetical order.

### `unset <namespace> <env>`

To remove a previously stored secret:

```console
$ divechain unset github GITHUB_TOKEN
```

`unset` deletes the secret for the exact `namespace` and `env` pair.
If no matching secret exists, the command prints an error to standard error and exits with a non-zero status.

### `exec <namespace> -- <command> [args...]`

To run another command with all secrets from a namespace injected as environment variables:

```console
$ divechain exec github -- gh auth status
Logged in to github.com account janedoe (...)
```

`exec` searches secrets whose have the specified `namespace`, loads every `env -> secret` pair stored under that namespace, adds them to the child process environment, and then replaces the current process with the requested command.
If no secrets are found for the namespace, the command still runs with the existing environment.

## Development

On macOS, the runtime CLI targets the default user keychain.
