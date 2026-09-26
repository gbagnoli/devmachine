# Skillet

Skillet is a Rust-based tool for idempotent host configuration management. It is designed to be highly modular, with core primitives in `skillet_core`, hardening modules in `skillet_hardening`, and host-specific binaries built on top of these.

## Building

Skillet builds fully static musl binaries by default (see `.cargo/config.toml`),
so each host binary runs on any x86_64 Linux target with no glibc or system
library dependencies — ideal for dropping onto an immutable host.

One-time setup:
```bash
rustup target add x86_64-unknown-linux-musl
```

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
The per-host binaries land at
`target/x86_64-unknown-linux-musl/release/skillet-<hostname>`
(e.g. `skillet-clamps`). Verify with `file`, which should report
"statically linked".

## Running

The tool provides an `apply` command to execute configuration. `apply` defaults
to the full host and requires its application credentials. Use `--phase base`
to converge only the shared host baseline before credentials are available.
- **Agent Mode**: Run generically on a host:
  ```bash
  ./target/x86_64-unknown-linux-musl/debug/skillet apply --host clamps --phase base
  ```
- **Host-Specific Configuration**: If a host-specific binary has been built (e.g., `skillet-beezelbot`), you can use it to apply specific configurations.

## Disposable VM smoke check

Build both static binaries from this directory:

```bash
cargo build --release -p skillet-clamps -p skillet
sha256sum target/x86_64-unknown-linux-musl/release/skillet-clamps
```

The host artifact is `target/x86_64-unknown-linux-musl/release/skillet-clamps`.
The current artifact SHA256 is
`55bf7584ce405dcf2c20956c182f4dbfe04bfbb20a9b649e48ab97877d9e8856`.
Supply that artifact to the guest, then invoke `skillet-clamps apply --phase base`.
The generic binary is used only by the disposable fixture smoke check:

```bash
integration_tests/smoke-ssh.sh \
  --target core@127.0.0.1 --disposable-target \
  --port 2201 \
  --identity ../butane/runs/clamps-test-example/ssh/id_ed25519 \
  --binary target/x86_64-unknown-linux-musl/release/skillet \
  --clamps-binary /var/lib/skillet/skillet-clamps
```

The target and disposable opt-in are mandatory. The script checks baseline/full
separation, container startup and idempotency, configuration and dummy secret
rotation, failed startup recovery, interrupted activation, and reboot
persistence. It writes state snapshots and failure output under
`/var/lib/skillet-smoke/` in the guest. Run it on a fresh disposable VM; its
fixture unit and data use only `skillet-smoke` names. The guest needs passwordless
sudo for the SSH user and access to pull `docker.io/library/alpine:3.20`.
When `--identity` is supplied, SSH stores that VM's host key beside the private key.

## Architectural Mandates
- **Error Handling**: Use `thiserror` in library crates; `anyhow` is reserved for CLI binaries. No `unwrap()` or `expect()` in library code.
- **Idempotency**: All modules must ensure system state idempotently.
- **System Interactions**: Prioritize Rust crates (e.g., `zbus`, `users`) over shelling out to system commands.
- **Linting**: Run `cargo clippy --workspace --all-targets -- -D warnings`; the workspace enables pedantic lints with documented exceptions.
