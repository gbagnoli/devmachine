# Project: ublue

## Overview

This is a Rust CLI project named `ublue`.
It uses:

- `clap` for command-line argument parsing.
- `anyhow` for error handling in the main entry point.
- `thiserror` for library-level error handling.
- `std::process::Command` to interact with system utilities
- `serde` for JSON serialization
- A library/binary split structure.

## Development Workflow

### Building

- Build: `cargo build`
- Run: `cargo run -- [args]` (e.g., `cargo run -- gsettings list`)

### Testing

- Run tests: `cargo test`

### Code Quality

- Format code: `cargo fmt`
- Check for errors: `cargo check`
- Run linter: `cargo clippy`
- Finish with a build: `cargo build`

## CLI Standards

- Format: All commands must support a `--format` option taking an enum of:
   - `human` (default)
   - `json`
   - `csv`
   - `tsv`

## Development Conventions

- Error Handling:
   - Use `anyhow` in `main.rs` and `thiserror` in `lib.rs` and other modules.
   - **NEVER** use `unwrap()` or `expect()`. Always propagate errors or handle them gracefully.
   - Library functions must return proper errors via `thiserror`.
