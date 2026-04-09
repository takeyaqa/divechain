# divechain

`divechain` is a Rust CLI for storing and retrieving secrets from platform credential stores.

The first backend is macOS Keychain, implemented with [`security-framework`](https://docs.rs/security-framework/latest/security_framework/).

## Usage

```console
$ cargo run -- set com.example.demo alice super-secret
$ cargo run -- get com.example.demo alice
super-secret
$ cargo run -- delete com.example.demo alice
```

`set` creates or updates a generic password item in the default user keychain.

## Development

On macOS, the runtime CLI targets the default user keychain. The real save/load/delete tests are marked `ignored` because they require an authorized keychain session; run them explicitly with `cargo test -- --ignored`.
