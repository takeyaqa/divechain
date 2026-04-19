# Agent Instructions for divechain

## Key Conventions

- **Git Workflow**: Always create a new branch from `main` before starting any task
- **Commit Messages**: Use Conventional Commits format (e.g., `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`, `ci:`)

## Project Context

- **Project Type**: This repository is a Rust CLI project
- **Platform Focus**: Runtime keychain behavior is macOS-focused, with the platform implementation gated behind `target_os = "macos"`

## Validation

- **Required Checks**: Run `cargo fmt --all --check`, `cargo build --locked --verbose`, and `cargo test --locked --verbose -- --test-threads=1` before finishing substantial changes
- **CI Environment**: CI runs on `macos-latest`, so macOS is the source of truth for runtime validation
