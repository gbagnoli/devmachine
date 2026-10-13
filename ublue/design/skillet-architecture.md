# Skillet architecture

Keep one canonical host declaration for composition, credentials, UI exposure,
boot expectations and acceptance. Generic and per-host binaries dispatch that
same declaration; synthetic fixtures are test targets, not production identities.

Separate service recipes, guest resource adapters, host composition, workstation
providers/vault operations, VM lifecycle and CLI parsing. Effects cross injected
capability boundaries for both observations and mutations. Pure validation,
rendering and composition use data/functions rather than extra traits. Libraries
use thiserror; CLI boundaries handle presentation. Shell adapters must not become
parallel lifecycle owners or contain programs embedded in Rust.

Account identity, container process identity, host ownership and namespace mapping
are distinct. File/directory ownership has one named-or-numeric contract; missing
identity/ranges fail rather than silently choosing IDs. See
[Podman policy](podman-runtime.md).

The Rust VM owner holds per-run locks and validates recorded UUID/disk/firmware
ownership around bounded guest operations. Native and Flatpak adapters share
that policy. Build-result APIs select Cargo-reported artifacts; capture evidence
and later verified deployments remain distinct. Compatibility imports are explicit
and nondestructive; cleanup preserves journals until owned resources are gone.
[VM lifecycle](smoke-vms.md) owns these details.

Workstation libraries own vault/provider mutations and credential delivery.
Guest apply receives explicit credential inputs, never a workstation vault or
interactive unlock flow. Verified SSH transfers arguments separately and keeps
secret-bearing stdin/output in memory with bounded I/O and zeroization.
[Secret delivery](secrets.md) and [host application](host-application.md).

Convergence distinguishes definition changes, activation, retry and no-op.
Applied receipts follow successful consuming effects, so interrupted activation
can retry. Omission/removal semantics are explicit and preserve application data.
Diagnostic recording stores sanitized attempted-operation outcomes, survives
failures and is not desired state or proof of idempotence.

Fast fixtures establish pure/resource contracts; adapters/fakes preserve actual
resource types, metadata, state and failure semantics. Real named VMs establish
runtime/reboot behavior. Fixture-only, native-backend and production acceptance
are separate evidence scopes; unavailable checks stay unverified.

Current follow-up acceptance: [refactor tracker](../plan/SKILLET-REFACTOR.md).
Historical implementation decisions/results: [archive](../plan/archive/README.md).
