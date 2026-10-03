# Skillet Project Constraints & Structure

This document defines the architectural mandates and project structure for `skillet`, a Rust-based idempotent host configuration tool.

## Core Mandates

### 1. Error Handling & Safety
- **Libraries MUST use `thiserror`** for custom error types.
- **Libraries MUST NOT use `anyhow`**. `anyhow` is reserved for the CLI binary only.
- **NEVER use `unwrap()` or `expect()`** in library code. All errors must be propagated and handled.
- **Prioritize Crates over Shell-out**: Use Rust crates (e.g., `users`, `nix`) for system interactions whenever possible instead of executing shell commands.
- **Never embed shell scripts in Rust string constants or construct shell programs inside Rust.** Use Rust APIs and focused subprocess calls for work owned by Skillet. If a shell script is truly needed, keep it in a standalone script file with its own interface.

### 2. Idempotency
- All resources (files, users, groups, etc.) must be **idempotent**.
- Before performing an action, check the current state (e.g., compare SHA256 hashes for files, check existence for users).
- Actions should only be taken if the system state does not match the desired state.
- Use the normal blocking `systemctl start` or `restart` command and inspect its
  result. Do not pass `--wait`: it waits for a started unit to stop and can hang
  on long-running services or `RemainAfterExit=yes` oneshot units.

### 3. Testing Strategy
- **Unit Tests**: Place unit tests in a `tests` submodule within each module's directory (e.g., `src/files/tests.rs`).
- **Separation**: Never put tests in the same `.rs` file as the implementation code. Reference them using `#[cfg(test)] #[path = "MODULE/tests.rs"] mod tests;`.
- **Abstractions**: Use Traits (e.g., `FileResource`, `SystemResource`) to allow for mocking in higher-level library tests.

### 4. Quality Control & Validation
- **Formatting & Linting**: Always run `cargo fmt` and `cargo clippy` after making changes to ensure code quality and consistency. **Clippy MUST be run with `pedantic` lints enabled (configured in `Cargo.toml`).**
- **Verification**: Always run both:
    - **Unit Tests**: `cargo test` across the workspace.
    - **Runtime Smoke**: Run `integration_tests/smoke-ssh.sh` against an explicitly named disposable VM with real systemd and Podman for affected container resources. Record the guest state snapshots and failure diagnostics.

## Local musl toolchain

- On this workstation, the musl cross compiler is installed under
  `/opt/x86_64-linux-musl-cross/bin`. Before compiling musl binaries, run:
  `export PATH=/opt/x86_64-linux-musl-cross/bin:$PATH`.
- Ensure this PATH is inherited by Cargo and its child build commands. Check
  the installed Rust target and compiler before reporting musl as unavailable;
  a missing binary artifact alone does not indicate a missing toolchain.

## Architecture and refactoring rules

Follow [the refactoring roadmap](../plan/SKILLET-REFACTOR.md) before further
feature milestones. These rules describe the target architecture; the roadmap
tracks existing violations and their migration, not completed implementation.

- Route effectful observations and mutations through the same explicit
  boundary. Recipe and orchestration code must not bypass an injected resource
  interface to inspect the live filesystem, account database, process
  environment, or runtime. Concrete adapters own those operations; load
  credentials and ambient configuration at the entry point and pass them in.
- Define traits around cohesive capabilities needed by consumers. Add an
  operation to the capability that owns it, not an unrelated omnibus trait.
  Use ordinary data and functions for pure validation, rendering, and static
  recipe composition; do not introduce a trait for every recipe.
- Keep one authoritative host capability declaration. Application composition,
  UI exposure, credential requirements, and provisioning eligibility must agree
  with it. Reject unknown profiles for profile-dependent operations.
- Keep binary entry points thin. Host composition, guest runtime adapters,
  workstation provisioning, and test fixtures have distinct responsibilities.
  Extract modules first and crates where dependency or deployment boundaries
  justify them; production dispatch must not include synthetic test profiles.
- Share delivery and lifecycle workflows across environments. Supply explicit
  policy and target inputs rather than duplicating algorithms or branching on
  particular deployment names. Keep secret values off command lines and out of
  manifests, recordings, and errors.
- Use a common ownership representation for named and numeric identities.
  Model application identity, host filesystem ownership, and namespace mapping
  separately. Existing numeric ownership must not require a named host account.
- Give common configuration fields one authoritative representation. Reject
  conflicts between typed fields and extension directives; preserve ordering
  where repeated directives are semantically ordered. Global runtime policy
  belongs to the host baseline, not an arbitrary container resource.
- State what every convergence result means. Distinguish definition changes,
  activation, recovery, and no-op outcomes. Persist applied state only after the
  consuming operation succeeds and retry pending activation after interruption.
- Define whether omission means leaving a resource unmanaged or ensuring its
  absence. Deactivation must be explicit and must preserve application data
  unless its deletion is authorized.
- Diagnostic recording must survive apply failures and describe operation
  outcomes without exposing payloads. Do not treat an attempted operation as
  proof of a change, success, or idempotence.
- Fakes and command stubs must honor production contracts for state, metadata,
  identities, and error/exit semantics. Unsupported behavior must fail explicitly
  rather than silently succeeding. Use shared contract checks for important
  adapters and fakes; a mocked runtime does not prove real runtime acceptance.
- Select build artifacts from Cargo's reported outputs and honor its target,
  profile, and configured target directory. Do not discover the new build by
  scanning potentially stale binaries at conventional paths.
- Compilation and artifact lookup belong to one operation. Consumers use
  that operation's reported executables and reject incomplete or ambiguous
  results; do not rebuild in one path and rediscover binaries in another.
- Reading retained state must not implicitly migrate it. Import legacy formats
  explicitly, preserve original evidence, and use atomic creation that cannot
  overwrite a concurrently created authoritative manifest.
- Serialize lifecycle mutations per run. A successful runtime query must
  establish absence; transport failure is not absence. Revalidate ownership
  after long external calls and address destructive runtime actions by the
  recorded immutable identity, not only a reusable name.
- Keep subprocess adapters focused: construct executable arguments and
  environments directly, observe results, and bound waits with diagnostics.
  Preserve supported workstation backends and static guest builds without
  installing tools as part of a refactor.
- CI must cover executable helpers regardless of filename extension and must
  run relevant checks when their shared inputs change. Keep routine CI free of
  private credentials and external API access; record live acceptance separately.

## Testing Philosophy

Skillet uses a multi-layered testing approach to ensure reliability and idempotency:

1.  **Trait-based Abstraction**: Core resources (`FileResource`, `SystemResource`) are defined as traits. This allows for easy mocking using `MockFiles` and `MockSystem` in unit tests.
2.  **Diagnostic Recorder**: `apply --record PATH` can capture attempted resource operations for investigation. Recording equality is not an acceptance check.
3.  **Real Runtime Smoke**: The disposable VM check exercises generated Quadlets, systemd startup, Podman secrets, configuration changes, repeated apply, failure recovery and reboot persistence. It compares managed bytes and metadata plus container identity and observed application state.

## Project Structure

The project is organized as a Cargo workspace:

```text
skillet/
├── Cargo.toml          # Workspace configuration
├── AGENTS.md           # This file (Project mandates)
└── crates/
    ├── core/           # skillet_core: Low-level idempotent primitives
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── files.rs      # File management (Traits + Impl)
    │   │   ├── files/
    │   │   │   └── tests.rs  # Unit tests for files
    │   │   ├── system.rs     # User/Group management
    │   │   └── system/
    │   │       └── tests.rs  # Unit tests for system
    │   └── tests/            # Integration tests
    ├── hardening/      # skillet_hardening: Configuration logic (modules)
    │   ├── src/
    │   │   ├── lib.rs        # Hardening logic using core primitives
    │   │   └── tests.rs      # Unit tests for hardening logic
    │   └── tests/
    ├── cli/            # skillet: The main binary executable
    │   └── src/
    │       └── main.rs       # CLI entry point (uses anyhow, clap)
    ├── cli-common/     # skillet_cli_common: Shared CLI logic
    └── hosts/
        └── beezelbot/  # skillet-beezelbot: Host-specific binary for beezelbot
```

## Module Design
- **Service Modules**: Service crates compose shared primitives and own their application configuration. Supporting libraries own cohesive runtime, host-profile, or workstation responsibilities.
- **Binary per Host**: Host binaries select a shared canonical profile. Keep their behavior consistent with the generic entry point; do not duplicate recipe composition between binaries.
- **Core Primitives**: Found in `skillet_core`, providing the building blocks for all modules.
