# divechain

`divechain` is a Rust CLI for storing secrets in platform credential stores.

The first backend is macOS Keychain, implemented with [`security-framework`](https://docs.rs/security-framework/latest/security_framework/).

## Usage

```console
$ cargo run -- --set aws AWS_ACCESS_KEY_ID
aws.AWS_ACCESS_KEY_ID:
```

`--set` creates or updates a generic password item in the default user keychain.
When `namespace` is `aws`, the macOS Keychain service name is `divechain-aws`.
The `ENV` argument is stored as the Keychain account attribute, so examples use uppercase environment-variable names.
When running in a TTY, the secret is read interactively without echo.

You can also pipe the secret in non-interactive environments:

```console
$ printf 'super-secret\n' | cargo run -- --set aws AWS_ACCESS_KEY_ID
```

## Development

The current implementation intentionally exposes only `--set`. On macOS, the runtime CLI targets the default user keychain.
