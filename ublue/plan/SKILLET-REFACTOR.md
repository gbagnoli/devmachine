# Skillet refactoring prerequisite

Status: planned, 2026-10-03. No implementation or new acceptance is claimed.
**Complete these seven workstreams before proceeding with further feature
milestones, service migrations, encryption implementation, or production
cutover.** Preserve the pending tasks in the existing plans. Acceptance and
repairs needed to validate this refactoring may proceed within its scope.

Read `../AGENTS.md`, `../skillet/AGENTS.md`, and
[the architecture decision](../design/skillet-architecture.md) first. Then read
the designs and source files listed in the selected workstream. Start from the
current working tree; do not assume the review's commits or line numbers are
still current. Inspect changes before editing and preserve unrelated work.

## Execution order

The numbers match the seven review findings. Complete one reviewable slice at
a time in this order. These are coordinated workstreams, not seven independent
assignments to execute concurrently against the same interfaces.

| Order | Workstream | Depends on | Status |
| --- | --- | --- | --- |
| 1 | [VM lifecycle and Bash-to-Rust migration](REFACTOR-01-VM-LIFECYCLE.md) | Current lifecycle/secret designs | Rust owns create and retained-run commands; required live acceptance remains |
| 2 | [Canonical host profiles](REFACTOR-02-HOST-PROFILES.md) | VM identity types from 1 | Implemented; live acceptance pending |
| 3 | [Effect interfaces and ownership](REFACTOR-03-RESOURCE-BOUNDARIES.md) | Profile inputs from 2 | Planned |
| 4 | [CLI/library and delivery boundaries](REFACTOR-04-CLI-PROVISIONING.md) | Interfaces from 1–3 | Planned |
| 5 | [Podman configuration and host policy](REFACTOR-05-PODMAN-MODEL.md) | Profiles/effect interfaces from 2–4 | Planned |
| 6 | [Convergence outcomes and recording](REFACTOR-06-CONVERGENCE-RECORDING.md) | Resource/configuration contracts from 3–5 | Planned |
| 7 | [Test layers, fixtures, artifacts, and CI](REFACTOR-07-TEST-INFRASTRUCTURE.md) | Final interfaces from 1–6 | Planned |

Each earlier slice includes its own regression checks; testing does not wait
for workstream 7. Workstream 1 creates the minimal VM/transport contracts used
by workstream 4; workstream 4 consolidates delivery on those contracts rather
than inventing another VM transport or lifecycle owner. Preserve the existing
public command syntax unless a required change is documented with migration.

## Scope and completion gate

- Keep existing service crates and canonical public/private configuration
  boundaries. Do not add new services, change storage layouts, broaden token
  scope, install workstation packages, or migrate production data in this work.
- Preserve supported native and Flatpak VM backends, static guest artifacts,
  retained VMs, SSH verification, atomic vault writes, and credential lifetimes.
- Routine CI uses dummy secrets and local fakes/HTTP fixtures. Live checks use
  named disposable VMs and the existing vault unlock workflow, never chat
  passwords or committed credentials. Preserve evidence and ownership on failure.
- For Rust slices run workspace format, pedantic Clippy, tests, and the current
  CI integration command; for shell slices run ShellCheck on every affected
  helper, including extensionless files. After workstream 7 use the updated CI
  commands. Run affected VM checks as each plan requires.
- Record exact commands, code/artifact versions, outcomes, and unavailable
  checks in `../butane/ACCEPTANCE.md`. Unavailable live/backend checks stay
  unverified; do not mark a workstream complete while its required evidence is
  missing. Distinguish unchanged, explicitly deferred production acceptance
  from the checks required to validate this refactor.
- Make focused commits after validation and document incomplete validation
  honestly. Do not push without authorization. Update this table and each
  workstream's progress, design decisions, README usage, and general AGENTS
  prevention rules in the corresponding implementation slice.
- Completion requires all seven exit criteria and a final fresh VM cycle:
  create → ready → provision → fixture and host acceptance → reboot → repeat
  apply → destroy, with external cleanup where resources were provisioned.
  Also verify retained-manifest compatibility and interrupted recovery.

Resume [the migration plan](CLAMPS-MIGRATION.md) only after this gate passes.
The existing private-UI, storage/credential, UniFi, and TPM plans retain their
unfinished work; this prerequisite changes their order, not their acceptance
requirements. Remove completed refactoring plans only after preserving decisions,
evidence, and any remaining tasks as required by AGENTS.
