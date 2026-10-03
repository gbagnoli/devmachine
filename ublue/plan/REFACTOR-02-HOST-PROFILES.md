# 2. Canonical host profiles and capabilities

Status: implementation complete; retained-VM acceptance is pending. Feature
milestones remain gated by [the prerequisite roadmap](SKILLET-REFACTOR.md).

## Read and locate

- `../skillet/crates/cli-common/src/{hosts,lib}.rs`, `hosts_tests.rs`, and
  `../skillet/crates/hosts/*/src/main.rs`.
- Workstream 1's manifest/identity types; workstation service checks in
  `../skillet/crates/cli/src/secret_delivery.rs` and CLI host validation.
- `../design/{private-ui-access,cloudflare-ui-lifecycle,secrets,storage,btrbk}.md`.

Current defects: application composition, UI declarations, credentials, and
delivery eligibility have separate host checks. Host binaries select shared
dispatch but do not own composition. Unknown full-apply profiles silently fall
back to the baseline, and UI presence is used as a proxy for service eligibility.

## Implementation sequence

Workstream 1 introduced explicit boot expectations in
`cli-common::hosts::boot_policy_for_host`. Fold this interim lookup into the
canonical profile declaration alongside composition and service capabilities;
keep the VM readiness orchestrator independent of deployment names.

1. Add canonical library `skillet_hosts` and move host definitions/composition
   out of `cli-common`. Define a validated `HostId` and `HostProfile` carrying
   enabled application configurations, shared network/storage policy, optional
   snapshot configuration, UI declarations, and credential requirements.
   Derive applicable UI/credential data from enabled services where possible;
   retain explicit aliases and exposure choices in the host declaration.
2. Preserve each current host's enabled services, paths, ports, snapshots,
   private configuration sources and environment policies. Remove redundant
   account setup only after workstream 3 establishes its purpose and consumers.
   Declare runtime identity separately from the profile selected for apply.
3. Keep one composition function per canonical profile, consumed by the generic
   CLI and per-host wrappers. Keep phase/domain logic independent of Clap;
   parse CLI values at entry points. Retain thin host binaries for compatibility
   while documenting their role. Do not duplicate recipes into the wrappers.
4. Have provisioning/delivery consult explicit service capabilities and their
   credential consumers. A service does not require an exposed UI merely to
   qualify for its own credentials. Unknown or unsupported profiles/services
   fail before vault unlock, remote mutation, or full apply. If a generic
   baseline is useful, make it explicit rather than an unknown-profile fallback.
5. Replace fixed identity checks in readiness and smoke selection with profile
   and run-manifest inputs from workstream 1. Keep test environment selection
   explicit. Preserve the current UI naming rule and ownership conflict refusal:
   instances sharing a host/environment DNS namespace cannot claim it together.
   Do not silently append an instance to served DNS names.
6. Remove duplicated registries and capability lists from CLI paths. Keep
   application-specific configuration in the service crate and host selections
   in the profile library. Document the authoritative source and dependencies
   in `../design/skillet-architecture.md` and update AGENTS genericity rules.

## Validation and exit criteria

- Table-driven tests cover every current profile's services, UI upstreams,
  aliases, credential consumers, network and snapshot selection. Test a profile
  with a service whose UI is unexposed and a profile with no UI/credentials.
- Generic and per-host entry points select the same canonical configuration.
  Full apply and delivery reject unknown profiles and undeclared services.
- A local synthetic declaration with a different identity proves shared lookup,
  readiness selection and rendering have no fixed-profile assumption. It is
  test data, not a synthetic production-dispatch host. No new real host needs
  to be bootstrapped merely to test genericity.
- Host/VM instance/environment/SSH target remain distinct; UI naming and
  DNS ownership conflicts preserve the current design and private values.
- Pass roadmap checks and affected retained-VM base/full apply checks. Record
  evidence and update profile callers, designs and usage before completion.

## Progress

- Implemented: `skillet_hosts` is the canonical profile and composition crate
  for clamps, beezelbot, and the explicit generic agent baseline. Profile data
  owns boot image/masks, enabled services, UI exposure, credentials, bridge
  network, storage requirement, and optional btrbk inputs.
- Implemented: generic and per-host apply wrappers, Caddy apply, credential
  eligibility, smoke selection, and VM boot expectations query the profile.
  Unknown profiles fail closed. The smoke fixture is a separate hidden test
  command, not a synthetic host profile.
- Pending: live retained-VM base/full apply checks and the roadmap fresh-VM
  cycle. These remain unverified until a named VM/runtime is available.
