# 3. Cohesive effect interfaces and unified ownership

Status: in progress; follows workstream 2 in
[the prerequisite roadmap](SKILLET-REFACTOR.md). Further feature milestones
wait for that roadmap's completion.

## Read and locate

- `../skillet/crates/core/src/{files,system,credentials,recorder,test_utils}.rs`.
- Account/subordinate-ID observations in `../skillet/crates/podman/src/lib.rs`
  and the host library; ownership inputs in application crates.
- `../design/{secrets,storage,skillet-architecture}.md` and workstreams 2 and 4.

Current defects: broad file/system traits accumulate unrelated capabilities,
but recipe code still reads live account data, subordinate-ID files and the
credential environment outside those interfaces. Named and numeric directory
ownership use separate APIs; mocks do not enforce important metadata contracts.

## Implementation sequence

1. Inventory effectful reads and writes outside concrete adapters. Assign each
   to its actual owner. Keep plain validation/rendering as functions, and avoid
   a general command-execution trait as a replacement for domain capabilities.
2. Split interfaces into cohesive file, storage, account lookup/ensure, service,
   and Podman-secret capabilities where consumers need them. Account identity
   and subordinate-range reads must be injectable together with account setup.
   Keep a transitional aggregate/context if needed to avoid a flag-day change;
   migrate recipe signatures and remove obsolete omnibus requirements afterward.
3. Define shared named/numeric ownership types for file and directory metadata.
   Resolve names through the account boundary or a concrete filesystem adapter,
   and preserve numeric IDs without requiring matching account names. Model
   container process identity and namespace mapping separately. Preserve the
   rule that application-data root convergence does not recursively change data.
4. Load systemd credentials and ambient configuration at the guest entry point.
   Pass validated credential inputs into host composition; do not construct the
   credential environment reader inside recipes. If a credential-reader trait
   is needed, keep it focused and provide an in-memory implementation for tests.
   Keep secret-value handling separate from recorded operations.
5. Move direct live account/file reads out of host/Podman recipes into adapters.
   Missing/invalid subordinate ranges must follow an explicit documented mapping
   policy, not an implicit hardcoded fallback. Audit user/group creation against
   real consumers; remove obsolete setup rather than keeping inert configuration
   merely for compatibility with an unused model.
6. Update concrete adapters, recorders and fakes together. Fakes preserve IDs,
   mode/ownership, file-versus-directory and subvolume distinctions, activity
   and failure semantics needed by callers. Unsupported operations fail clearly.
   Do not claim new trait fidelity by testing only that a mock was called.

## Validation and exit criteria

- Full host composition runs with injected dummy inputs and fakes without
  consulting the test runner's accounts, subordinate-ID files, environment,
  filesystem or services. Tests vary observations to prove decisions change.
- Contract cases cover absent/existing/wrong-ID accounts, invalid/missing ranges,
  named and numeric ownership, mode drift, wrong object type and failed effects.
  Include an unmapped numeric owner and verify descendants remain untouched.
- Core filesystem tests exercise a real temporary directory; privileged
  ownership/mapping and Btrfs behavior use an explicitly named disposable VM.
  Preserve mount guards and namespace behavior; do not weaken checks for CI.
- Static audit finds no live effect observations in composition/recipes that
  bypass their declared boundary. Traits have cohesive consumers and the two
  ownership APIs have become one contract.
- Pass roadmap checks and affected VM ownership/mount/repeat-apply checks.
  Update the design and generic AGENTS interface rules with the finished API.

## Progress

- Implemented: host-profile and Podman composition use injected account
  lookups; NSS access remains in the Linux system adapter. Podman reads
  subordinate-ID files through `FileResource`, validates the selected single
  range, and fails closed for missing, invalid, duplicate, or undersized ranges.
- Implemented: directory ownership uses one `Ownership` value with named or
  numeric UID/GID. Numeric IDs do not require matching account names. The
  fake preserves mode and ownership across existence-only checks; the UniFi
  repeat-apply case verifies child data survives.
- Implemented: the guest CLI loads phase-required systemd credentials and
  passes `CredentialInputs` to host composition. Recipes no longer inspect
  `CREDENTIALS_DIRECTORY`; required names derive from profile consumers.
- Implemented: file effects now expose separate read, mutation, and storage
  capabilities. Recipe signatures use those capabilities where they do not
  need the entire file interface. The aggregate remains for host composition
  and migration paths. Recorder and mock adapters implement the capabilities
  separately.
- Implemented: system effects expose separate account lookup, account
  mutation, Podman-secret, and service capabilities. The Podman, hardening,
  service, and backup crates request their required capabilities; host
  composition retains the aggregate during migration. Recorder and mock
  adapters implement the narrow contracts.
- Pending: shared adapter contract tests still need review, and live
  ownership/mount acceptance is outstanding. This workstream is not complete.
