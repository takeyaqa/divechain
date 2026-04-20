# divechain

`divechain` is a CLI for running commands with secrets from the macOS Keychain injected as environment variables.

## Usage

### `set <NAMESPACE> <ENV>`

```console
$ divechain set github GITHUB_TOKEN
github.GITHUB_TOKEN: 
```

`set` creates or updates a secret in the default user keychain. When `namespace` is `github`, the macOS Keychain service name is `divechain-github`. Saved macOS Keychain items also use the fixed Keychain label `divechain`. When running in a TTY, the secret is read interactively without echo.

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

`list` searches for secrets whose Keychain label is `divechain`, extracts the namespace, removes duplicates, and prints namespaces only, one per line, in alphabetical order.

### `unset <NAMESPACE> <ENV>`

To remove a previously stored secret:

```console
$ divechain unset github GITHUB_TOKEN
```

`unset` deletes the secret for the exact `namespace` and `env` pair. If no matching secret exists, the command prints an error to standard error and exits with a non-zero status.

### `exec <NAMESPACE> -- <COMMAND>...`

To run another command with all secrets from a namespace injected as environment variables:

```console
$ divechain exec github -- gh auth status
Logged in to github.com account janedoe (...)
```

`exec` searches for secrets with the specified `namespace`, loads every `env -> secret` pair stored under that namespace, adds them to the child process environment, and then replaces the current process with the requested command. If no secrets are found for the namespace, the command prints an error to standard error and exits with a non-zero status instead of running the command.

### `server --socket-path <PATH>`

To serve secrets to other processes on the same host over a Unix domain socket:

```console
$ divechain server --socket-path /tmp/divechain.sock
listening on /tmp/divechain.sock
```

The server accepts one request per connection, reads it to EOF, loads every secret stored under the requested namespace, writes one response, and closes the connection. This transport is synchronous and blocking. It is intended for environments such as containers that cannot access the macOS Keychain directly but can reach a Unix domain socket exposed by the host.
If the socket path already exists and is a stale Unix socket left behind by a previous server instance, `server` removes it and binds successfully. Active socket listeners and non-socket filesystem entries still cause the command to fail.

### `client-exec <NAMESPACE> [--socket-path <PATH>] -- <COMMAND>...`

To run a command with secrets fetched from a running `divechain server` instance:

```console
$ divechain client-exec github --socket-path /tmp/divechain.sock -- gh auth status
Logged in to github.com account janedoe (...)
```

If `--socket-path` is omitted, `client-exec` falls back to the `DIVECHAIN_SOCKET_PATH` environment variable:

```console
$ export DIVECHAIN_SOCKET_PATH=/tmp/divechain.sock
$ divechain client-exec github -- gh auth status
Logged in to github.com account janedoe (...)
```

If both are set, `--socket-path` takes precedence. If the server returns an error for the requested namespace, `client-exec` prints the error and exits without running the command.

## Development

On macOS, the runtime CLI targets the default user keychain.
