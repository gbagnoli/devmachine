# Skillet Project Constraints & Structure

This document defines the architectural mandates and project structure for `skillet`, a Rust-based idempotent host configuration tool.

## Core Mandates

### 1. Error Handling & Safety
- **Libraries MUST use `thiserror`** for custom error types.
- **Libraries MUST NOT use `anyhow`**. `anyhow` is reserved for the CLI binary only.
- **NEVER use `unwrap()` or `expect()`** in library code. All errors must be propagated and handled.
- **Prioritize Crates over Shell-out**: Use Rust crates (e.g., `users`, `nix`) for system interactions whenever possible instead of executing shell commands.

### 2. Idempotency
- All resources (files, users, groups, etc.) must be **idempotent**.
- Before performing an action, check the current state (e.g., compare SHA256 hashes for files, check existence for users).
- Actions should only be taken if the system state does not match the desired state.

### 3. Testing Strategy
- **Unit Tests**: Place unit tests in a `tests` submodule within each module's directory (e.g., `src/files/tests.rs`).
- **Separation**: Never put tests in the same `.rs` file as the implementation code. Reference them using `#[cfg(test)] #[path = "MODULE/tests.rs"] mod tests;`.
- **Abstractions**: Use Traits (e.g., `FileResource`, `SystemResource`) to allow for mocking in higher-level library tests.

### 4. Quality Control & Validation
- **Formatting & Linting**: Always run `cargo fmt` and `cargo clippy` after making changes to ensure code quality and consistency. **Clippy MUST be run with `pedantic` lints enabled (configured in `Cargo.toml`).**
- **Verification**: Always run both:
    - **Unit Tests**: `cargo test` across the workspace.
    - **Integration Tests**: `skillet test run <hostname>` for affected hosts to verify end-to-end correctness in a containerized environment.

## Testing & Recording Philosophy

Skillet uses a multi-layered testing approach to ensure reliability and idempotency:

1.  **Trait-based Abstraction**: Core resources (`FileResource`, `SystemResource`) are defined as traits. This allows for easy mocking using `MockFiles` and `MockSystem` in unit tests.
2.  **The Recorder Wrapper**: A `Recorder<T>` wrapper can be applied to any resource implementing these traits. It intercepts all operations (e.g., `EnsureFile`, `ServiceRestart`), records them into a sequence of `ResourceOp` enums, and then passes the call to the underlying implementation.
3.  **Containerized Integration Tests**:
    *   **Record Mode**: The tool runs against a fresh container, and the `Recorder` saves the sequence of operations to a YAML file (e.g., `integration_tests/recordings/beezelbot.yaml`).
    *   **Run Mode**: The tool runs again, and the *actual* sequence of operations is compared against the *recorded* one. Any mismatch (e.g., a missing service restart or an extra file write) causes the test to fail.
    *   **Idempotency**: By running the tool twice in the same container, we can verify that the second run produces an empty (or minimal) set of operations, confirming idempotency.

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
- **Modules as Cookbooks**: Each library crate under `crates/` (besides `core`) represents a "module" or "cookbook" (e.g., `skillet_hardening`).
- **Binary per Host**: The idea is to have one binary per host type that picks up these modules and reuses core primitives.
- **Core Primitives**: Found in `skillet_core`, providing the building blocks for all modules.
