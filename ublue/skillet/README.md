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

The legacy CI command `skillet test run beezelbot --image fedora:latest` runs
the host apply twice in a uniquely named, temporary Podman container. It
builds the selected host binary, checks both applies succeed, and checks the
second apply does not start or restart a service. `clamps` is also supported.
This fast sandbox uses mock `systemctl` and `podman` executables, so it checks
apply behavior and repeat convergence but does not establish real systemd or
Podman operation. `--inspect` opens the container before cleanup.

For real systemd/Podman and reboot coverage, create a disposable VM first:

```bash
cargo run --release -p skillet -- test vm create
```

This provisions `clamps-test-smoke` on `giacomo@127.0.0.1:2201`, creates its SSH
key, and waits for the guest to pass `test-vm clamps-test-smoke ready`. The same command
accepts another host when its `HOST.bu` and `skillet-HOST` are available.
Then run the clamps smoke scenario:

```bash
cargo run --release -p skillet -- test smoke clamps
```

The smoke command defaults to that target, port, and generated key. Override
them with `--target`, `--port`, and `--identity` when using a separately
provisioned VM. It takes the generic binary from the running executable and
uses the `skillet-clamps` binary installed by VM creation. Repeated smoke runs
reset only the namespaced `/var/lib/skillet-smoke` fixture and its managed
files, so you can inspect a run and rerun it on the same disposable VM.

When finished, remove the disposable VM, disk, SSH key, and run artifacts:

```bash
cargo run --release -p skillet -- test vm destroy
```

The VM helpers require the host setup documented in `../butane/README.md`.
The VM fixture controls its own image. Smoke still requires the Skillet
workspace and Cargo so it can build the host binary alongside the running
executable.

Build both static binaries from this directory if you want to install the host
binary separately:

```bash
cargo build --release -p skillet-clamps -p skillet
sha256sum target/x86_64-unknown-linux-musl/release/skillet-clamps
```

The host artifact is `target/x86_64-unknown-linux-musl/release/skillet-clamps`.
Supply that artifact to the guest, then invoke `skillet-clamps apply --phase base`.

The smoke scenario checks baseline/full separation, container startup and
idempotency, configuration and dummy secret rotation, failed startup recovery,
interrupted activation, and reboot persistence. It writes state snapshots and
failure output under `/var/lib/skillet-smoke/` in the guest. It resets only its
own fixture files before each run, so repeated runs on the same disposable VM
are supported. The guest needs passwordless sudo for the SSH user and access to
pull `docker.io/library/alpine:3.20`. When `--identity` is supplied, SSH stores
that VM's host key beside the private key.

## Architectural Mandates
- **Error Handling**: Use `thiserror` in library crates; `anyhow` is reserved for CLI binaries. No `unwrap()` or `expect()` in library code.
- **Idempotency**: All modules must ensure system state idempotently.
- **System Interactions**: Prioritize Rust crates (e.g., `zbus`, `users`) over shelling out to system commands.
- **Linting**: Run `cargo clippy --workspace --all-targets -- -D warnings`; the workspace enables pedantic lints with documented exceptions.
