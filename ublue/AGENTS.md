# uBlue agent instructions

## Current priority

- Complete the seven workstreams in [the Skillet refactoring roadmap](plan/SKILLET-REFACTOR.md)
  before proceeding with further service migrations, encryption features, or
  production cutover. VM management's Bash-to-Rust migration is included.
- Follow the roadmap's ordering and validation gates. Existing feature plans
  retain their unfinished work but are gated by this prerequisite. Fixes and
  acceptance work necessary to complete the refactoring are in scope.

## Design documentation

- Read the relevant documents in [design/](design/) before changing architecture
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

- Keep implementation plans in [plan/](plan/), not in the `ublue/` root.
- Read relevant plans before starting work and update their remaining steps,
  status, and links as work progresses. Plans describe tasks and acceptance
  criteria; architectural decisions and rationale belong in `design/`.
- Remove completed or superseded plans after preserving any unfinished work
  in an active plan. Retain implementation evidence in the relevant acceptance
  record; do not label unverified checks as passed.
- Each refactoring handoff must name the implemented slice, remaining work,
  interface changes, and validation evidence. Record the general prevention
  rule in the applicable `AGENTS.md` when an architectural defect is corrected;
  keep incident-specific details in designs, plans, or acceptance records.

## Shared infrastructure boundaries

- Shared helpers receive identity and configuration from validated inputs or
  declarations. Keep deployment-specific assumptions in the selected profile;
  do not bake them into generic readiness, delivery, or lifecycle code.
- Give each resource lifecycle one owner. Every public entry point must use
  the same ownership validation, recovery, and cleanup policy; transport
  adapters must not provide a shortcut that bypasses those guarantees.
- Keep shell entry points as narrow argument adapters when Rust owns a
  lifecycle. Backend selection, resource inspection, retries, and cleanup
  belong to the shared Rust owner rather than being duplicated in wrappers.
- Keep host profile, environment, deployment instance, runtime identity, and
  connection target distinct. Derive conventions in one place and persist the
  resolved values needed for recovery.
- Persist ownership before mutations and preserve it across partial failures.
  Cleanup must tolerate already-absent owned resources while refusing ambiguous
  or unrelated resources. Remove recovery metadata only after cleanup completes.
- Compatibility-sensitive refactors must preserve retained artifacts and
  manifests through an explicit migration; never infer that existing state is
  disposable merely because its format is old.

## Local validation before commit

- Before committing, run the CI checks that apply to the changed paths locally
  and resolve failures first. For Skillet changes, run
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --all-targets`, and
  `cargo run --bin skillet -- test run beezelbot --phase base --image fedora:latest`
  from `.github/workflows/ci.yml`. For shell changes, run the workflow's
  ShellCheck commands; for Python or Chef changes, run the corresponding
  Ruff/Mypy or Cookstyle commands.
- For changes to container behavior, also run the affected host's real-runtime
  smoke test against a named disposable VM as required by `skillet/AGENTS.md`.
- If a required check cannot run because a tool, runtime, or external service
  is unavailable, record the exact reason and report the check as unverified;
  do not describe it as passing or commit as though it passed.
