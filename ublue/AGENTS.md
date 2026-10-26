# uBlue agent instructions

## Current work and acceptance

- Use [the active plan index](plan/README.md) for unfinished work and
  [refactor acceptance](plan/SKILLET-REFACTOR.md) for its parked checks.
  Implementation history belongs in `plan/archive/`; preserve unverified gates
  without treating them as blockers for independent user-authorized work.
- Follow each active plan's ordering and acceptance scope. Native, Flatpak,
  fixture, provider and physical results are distinct; preserve original evidence.

## Design documentation

- Keep `skillet/README.md` limited to build, brief test instructions and numbered
  physical installation steps. Put module setup/commands/troubleshooting in its
  owning crate README; use a focused `docs/` page under the existing owner when
  there is no dedicated crate. Maintain the module index and links when moving
  instructions; do not create crates solely to house documentation.
- READMEs describe current operation, designs record decisions/rationale, active
  plans describe unfinished work/acceptance, and `plan/archive/` holds completed
  plans and dated history. Append evidence to `plan/archive/ACCEPTANCE.md`.
  Archive superseded instructions before removing them from active documents;
  do not copy status timelines or task lists into setup guides or designs.

- Maintain the repository-root [secrets checklist](../SECRETS.md) whenever a
  vault entry or secret requirement is added, changed, or removed. Group entries
  by module in alphabetical order. Keep explanations brief; include the exact
  path, required/optional/generated status, where to obtain the value or a short
  setup link, and whether it belongs in Username or Password. Distinguish planned
  requirements from implemented ones; never include real credentials or private
  identifiers.
- The checklist and `secrets check` share
  `skillet/crates/workstation/src/secret-requirements.json`. Update that metadata
  and regenerate `SECRETS.md` with `cargo run -p skillet_workstation --example
  secrets_documentation` from the Skillet workspace (redirect output to the
  repository-root file). Keep template-reference coverage and the documentation
  equality test passing. Required checks follow selected host capabilities and the actual environment
  delivery policy; isolated test modes must not require unused provider credentials.
  Structured vault entries must declare each required standard/custom field and
  its validation in the shared audit metadata; keep single-value entry behavior
  compatible, and never include field values in audit output or errors.
  Unused-entry auditing must consider all declared hosts and both environments,
  including optional/generated entries and retained migration readers.
  Selected-scope validation must expand declared instance patterns without
  consulting another environment, and retain collection-specific schemas rather
  than applying single-value Password defaults to structured collections.

- Read the relevant documents in [design/](design) before changing architecture
  or behavior they describe, including [secrets](design/secrets.md) and
  [smoke VM lifecycle](design/smoke-vms.md).
- Keep those documents updated in the same change as the implementation or
  decision they describe. Clearly distinguish planned from implemented behavior.
- Record new architectural decisions in `ublue/design/`, updating an existing
  document when appropriate or adding a short document for a new topic.
- Keep design documents brief: describe the decision, why it was chosen, and
  material consequences. Keep setup and usage instructions in READMEs, with
  links to the relevant designs instead of duplicated rationale.

## Implementation plans

- Keep implementation plans in [plan/](plan), not in the `ublue/` root.
- Read relevant plans before starting work and update their remaining steps,
  status, and links as work progresses. Plans describe tasks and acceptance
  criteria; architectural decisions and rationale belong in `design/`.
- Retire completed or superseded plans into `plan/archive/` after preserving unfinished work
  in an active plan. Retain implementation evidence in the relevant acceptance
  record; do not label unverified checks as passed.
- Each refactoring handoff must name the implemented slice, remaining work,
  interface changes, and validation evidence. Record the general prevention
  rule in the applicable `AGENTS.md` when an architectural defect is corrected;
  keep incident-specific details in designs, plans, or acceptance records.

## Shared infrastructure boundaries

- Management connectivity changes require a separate explicit activation
  boundary. Staging preserves the current control connection. Boot preparation
  avoids network-online dependency cycles, retains a recovery profile, and keeps
  physical network inputs out of disposable provisioning.

- Disabling application authentication requires explicit proxy authorization
  and a private backend boundary. Preserve the actual connection peer, replace
  untrusted identity headers, and deny unknown/tagged sources or lookup failures.
  Sharing a network address range is not user authentication. Test policy
  revocation, daemon outages, browser origin checks, and direct backend isolation.
  Verify effective forwarding isolation in both IP families; network flags can
  change whether the runtime generates firewall rules at all.
- Concurrent boot consumers that mutate a process-global resource must serialize
  those mutations explicitly. Verify the full production ingress path after
  reboot; a local fixture proxy cannot establish that the deployed proxy started.
- Shared application login must select one explicit account behind the same
  per-request proxy authorization. Preserve client addresses, exclude direct
  backend access and retain local recovery; an existing application session
  must not bypass later proxy-policy revocation.
- Identity/private policy follows the same canonical UI declaration and encrypted
  delivery path as the service. Give proxy processes only the API surface they
  consume; a read-only Unix socket bind does not limit API methods.

- Authentication finalization must follow explicit provisioning completion, not
  base readiness. One-time account mutations need durable recovery state and
  must preserve later operator password changes; routine apply must not reopen
  temporary privileged access or repeat password expiry.

- Hardware-dependent services use an injected observation boundary. Converge
  configuration without absent-device startup failures, order startup after
  device discovery, and preserve existing outage handling when hardware vanishes.
  Validate simulated shutdown paths only in isolated disposable state.

- Optional services require explicit selection in the canonical host caller
  declaration. Shared baseline services retain their common defaults; the
  presence of environment configuration or credentials must not enable an
  optional service implicitly.
- Service modules own their monitoring declarations. Monitoring consumes the
  same selected services as application composition; do not maintain an
  independent monitoring inventory or place credentials in discovery metadata.
- Generated unit executable paths must agree with the actual bootstrap and
  update delivery paths. Cover that agreement in regression checks; consumer
  placeholders alone cannot establish deployment correctness.
- Native metric collectors use bounded read-only queries and a finite metric/tag
  inventory. Failed observations must report failure rather than synthetic zero
  readings. Keep ingestion credentials in the Agent and explicitly scope local
  submission endpoints. Validate collector output with the actual receiving
  implementation; local submission does not establish remote ingestion.

- Shared helpers receive identity and configuration from validated inputs or
  declarations. Keep deployment-specific assumptions in the selected profile;
  do not bake them into generic readiness, delivery, or lifecycle code.
- Keep clear configuration policy in reviewed source templates and secret
  leaves in the vault. Resolve secrets through typed references at the
  workstation boundary; do not store whole configuration documents in the
  vault or interpolate secret text into configuration strings. Record template
  behavior and schema changes in the relevant design document.
- Scope vault credentials to their actual provider/account needs; do not
  require separate copies by environment when a shared credential is intended.
  Keep public endpoint/site choices in configuration, with documented defaults.
- Give each resource lifecycle one owner. Every public entry point must use
  the same ownership validation, recovery, and cleanup policy; transport
  adapters must not provide a shortcut that bypasses those guarantees.
- Removing an optional generated configuration section must tolerate its
  absence, while rejecting malformed present sections. Cover fresh inputs and
  retained compatibility inputs through the actual staging/compiler path.
- Physical installation media requires explicit destination disk identity and
  private bootstrap access. Keep media generation separate from block-device
  writes; validate hardware prerequisites and confirm destruction on the target.
  Destructive confirmation may use a displayed short name only to acknowledge
  the already validated resource; it must not select or redirect the target.
  Respect installer metadata size limits; bind separately delivered installation
  payloads to the prepared media and verify them before destructive operations.
  Preserve virtual installation support without leaking virtual disk assumptions
  into physical profiles. Never describe generated media as hardware acceptance.
- Installation profiles are explicit and independent of host identity. Preserve
  compatible defaults and reject missing prerequisites rather than silently
  weakening a requested security policy. Convergence never reformats live data.
- Trust predicates must verify explicit positive evidence as well as supported
  tool statuses; pending requests, blocked resources and unknown results must
  fail closed. Cover supported consumer-version semantics in regression checks.
- Preserve downstream schema types when parsing and re-emitting configuration.
  Validate staged output with the actual consumer/compiler where possible, not
  only with a parser round trip.
- Do not assume package directories under mutable OS paths are initialized by
  an image build or rebase. Converge required state directories, ownership and
  security labels before confined service startup; test from absent package state.
- Keep credential staging distinct from activation. A narrowly named delivery
  command must not implicitly activate unrelated workloads; complete host
  application belongs to explicit orchestration driven by canonical declarations.
  Preserve strict host trust when adopting normal workstation SSH defaults, and
  retain explicit run-owned credentials for disposable targets.
- Before activating a unit with multiple credential prerequisites, deliver the
  complete credential set first. Cover activation from a fresh host where the
  consumer service and its containers do not yet exist.
- Keep shell entry points as narrow argument adapters when Rust owns a
  lifecycle. Backend selection, resource inspection, retries, and cleanup
  belong to the shared Rust owner rather than being duplicated in wrappers.
- VM guest operations hold the per-run lock and revalidate the recorded domain
  identity immediately before and after each remote operation. Consumers load
  target details from the typed manifest; they do not launch compatibility
  wrappers for status checks or reparse legacy run files.
- Keep host profile, environment, deployment instance, runtime identity, and
  connection target distinct. Derive conventions in one place and persist the
  resolved values needed for recovery.
- File and directory mutations use the same typed ownership contract for
  named and numeric UID/GID values. Numeric ownership must not require a host
  account lookup; preserve omitted metadata fields on existing paths.
- Services sharing bind-mounted data must resolve one host data identity and
  use it consistently for consumer workers, writable directories and secret-file
  permissions. Do not assume host account IDs match an image's defaults.
- Recovery acceptance preserves the original unlock path and uses independently
  owned disposable state for destructive tests. Disable diagnostic capture on
  credential-capable boot consoles; keep recovery backups outside the protected
  resource and verify their bytes after encrypted persistence.
- Persist independently usable recovery credentials before enrolling them. Bind
  recovery records to the resource identity, retain pending state for retries,
  and refuse silent replacement or re-enrollment of verified credentials.
- Verify serialized replacement as well as initial insertion. Preserve unrelated
  data and historical/shared references; reject a mutation when the persistence
  library cannot preserve them. Cover repeated updates through actual readback.
- Persist ownership before mutations and preserve it across partial failures.
  Resolve existing API identities before constructing validation/test requests;
  test the actual merged update, including retained fields and resource IDs.
  Treat redacted secrets as opaque: a mask alone cannot prove the configured
  credential is correct. Use supported consumer validation before preserving it.
  Cleanup must tolerate already-absent owned resources while refusing ambiguous
  or unrelated resources. Remove recovery metadata only after cleanup completes.
- Compatibility-sensitive refactors must preserve retained artifacts and
  manifests through an explicit migration; never infer that existing state is
  disposable merely because its format is old.

## Local validation before commit

- Compare CI and local Rust/Clippy versions when diagnosing lint failures.
  Validate fixes with CI's toolchain version; an older local toolchain passing
  does not verify lints added in a newer release.
- Before committing, run the CI checks that apply to the changed paths locally
  and resolve failures first. For Skillet changes, run
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --all-targets`, and
  `cargo run --bin skillet -- test run joey --phase base --image fedora:latest`
  from `.github/workflows/ci.yml`. For shell changes, run the workflow's
  ShellCheck commands; for Python or Chef changes, run the corresponding
  Ruff/Mypy or Cookstyle commands.
- When Butane bootstrap inputs or tests change, also run
  `bash ublue/butane/tests/bootstrap.sh` from the repository root. CI must
  execute state-regression scripts as tests, not only lint their source.
- For changes to container behavior, also run the affected host's real-runtime
  smoke test against a named disposable VM as required by `skillet/AGENTS.md`.
- If a required check cannot run because a tool, runtime, or external service
  is unavailable, record the exact reason and report the check as unverified;
  do not describe it as passing or commit as though it passed.

- Keep canonical host spelling consistent across profile, package, binary,
  installation templates and image. Identity renames require an explicit
  compatibility decision; never silently rewrite retained manifests or vault
  entries. Preserve historical acceptance evidence as recorded.

- Render dual-stack port exposure using non-overlapping runtime bindings.
  Preserve explicit address/family restrictions and validate forwarding cleanup
  across repeated restarts; successful initial connectivity alone is insufficient.

- Host/container Unix socket integration must validate process-domain connect
  permissions as well as inode labels and Unix ownership. Prefer a dedicated
  confined broker domain with explicit permissions; do not grant containers
  access to arbitrary host daemons or disable enforcement to fix a denial.

- Authentication subrequests must explicitly normalize their method/path/query
  without altering the original application request. Validate the consumer's
  adapted configuration and query-bearing API/assets, rather than testing only
  a successful HTML root response.

- Prepare declared storage identities before every first consumer, including OS
  image import and early background jobs. Application service ordering alone
  cannot prevent another consumer from creating an incompatible ordinary directory.
  Existing incompatible state requires explicit inspection/recovery, never silent
  conversion or deletion during ordinary convergence.

- For artifacts shared between services, verify consumer traversal and read access
  along the complete declared path and its namespace mapping. Leaf ownership alone
  is insufficient. Converge only declared ancestors; preserve unrelated trees and
  verify access through the actual consuming service.
- Initialization acceptance must cross the consumer's configuration confirmation
  or rollback window and a restart. A successful first request does not prove
  durable settings. Use the consumer's own initialization/schema APIs and preserve
  existing application-managed state on repeat provisioning.
