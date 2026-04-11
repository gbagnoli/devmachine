# Skillet

Skillet is a Rust-based tool for idempotent host configuration management. It is designed to be highly modular, with core primitives in `skillet_core`, hardening modules in `skillet_hardening`, and host-specific binaries built on top of these.

## Building

### Development Build
To build the workspace for development, use the standard cargo command:
```bash
cargo build
```

### Production Build
For optimized production builds, use the `--release` flag:
```bash
cargo build --release
```

## Running

The tool provides an `apply` command to execute configuration.
- **Agent Mode**: Run generically on a host:
  ```bash
  ./target/debug/skillet apply
  ```
- **Host-Specific Configuration**: If a host-specific binary has been built (e.g., `skillet-beezelbot`), you can use it to apply specific configurations.

## Testing

Skillet uses containerized integration tests to verify idempotency and state changes.

### Running Integration Tests
To verify an existing recording for a host (e.g., `beezelbot`):
```bash
./target/debug/skillet test run beezelbot --image fedora:latest
```

### Recording Integration Tests
To record a new configuration state for a host:
```bash
./target/debug/skillet test record beezelbot --image fedora:latest
```

You can append `--release` to these commands to test against production-optimized binaries.

## Architectural Mandates
- **Error Handling**: Use `thiserror` in library crates; `anyhow` is reserved for CLI binaries. No `unwrap()` or `expect()` in library code.
- **Idempotency**: All modules must ensure system state idempotently.
- **System Interactions**: Prioritize Rust crates (e.g., `zbus`, `users`) over shelling out to system commands.
- **Linting**: All builds must pass `cargo clippy --pedantic`.
