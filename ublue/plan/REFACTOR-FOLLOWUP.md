# Refactor follow-up: close the completion audit gaps

Status: implementation and local VM acceptance substantially complete,
2026-10-05. Code and local regression coverage for all six findings are in
place. Rust-owned fixture smoke, retained-VM update, and scripted interrupted
reboot recovery passed on the named clamps VM. Syncthing now declares its host
owner separately from its container UID/GID and maps them with the shared
Podman user-namespace API. Its live application acceptance must be rerun before
considering this finding resolved. Remote CI evidence remains pending.

## Progress

| Step | Implementation | Evidence still needed |
| --- | --- | --- |
| 1. Rust-owned smoke lifecycle | Implemented; named `clamps/smoke` fixture/reboot and scripted interrupted-checkpoint recovery passed | None locally; remote CI pending |
| 2. Update delivery guard | Implemented; ownership-loss/retry tests and retained-VM update passed | None locally; remote CI pending |
| 3. Fake contracts | Implemented; workspace regression suite passes | None locally; remote CI pending |
| 4. Bootstrap CI | Implemented; CI runs state regressions and local command passes | Observe remote CI after push |
| 5. Host application acceptance | Implemented from canonical profile capabilities; Syncthing host/container identity mapping corrected with regression coverage | Named-VM Syncthing apply, repeat apply, and reboot acceptance after the mapping change |
| 6. Documentation | Updated implementation, commands, design status, and fault-injection evidence | Final review and remote CI result |

## Start here

Read `../AGENTS.md`, `../skillet/AGENTS.md`, `SKILLET-REFACTOR.md`,
`REFACTOR-07-TEST-INFRASTRUCTURE.md`, and the architecture, smoke-VM, secrets,
and private-UI designs. Inspect current source and working-tree changes before
editing; file locations below identify responsibilities, not frozen interfaces.

Work in the order below, with focused commits after validation. Keep public
command syntax and retained manifests compatible. Use existing native/Flatpak
backends and toolchains; install nothing on the workstation. Libraries use
`thiserror`; never embed shell programs in Rust. Routine tests need no vault,
provider access, or hypervisor. Live checks use an explicitly named disposable
VM and the existing vault-unlock workflow. If unlock would block, continue
independent work and record the live check as pending.

## 1. Move smoke operations under the Rust lifecycle owner

Inspect `skillet/crates/cli/src/main.rs` (`run_smoke`, `SmokeLifecycle`),
`crates/vm/src/{transport,readiness,backend,manifest}.rs`, and
`integration_tests/smoke-guest.sh`. The former `smoke-ssh.sh` lifecycle owner
has been removed.

- Hold the per-run lock for the complete smoke workflow. Load SSH trust,
  identity, phase, and deployed artifact hashes from the validated manifest.
  Keep optional CLI overrides as equality checks against those values.
- Use `OwnershipCheckedTransport` for every upload and guest command. Validate
  live UUID/disk ownership after artifact compilation and before guest contact.
  Refuse later remote operations as soon as ownership validation fails.
- Move fixture/script delivery, installed-artifact verification, reboot, and
  bounded return detection into shared Rust orchestration. Guest assertion
  scripts may remain standalone; they receive explicit validated inputs and
  must not own SSH, lifecycle retries, or reboot.
- Persist the non-ready checkpoint before reboot. Verify that a new boot
  occurred and the selected profile's readiness checks pass before restoring
  Ready. Preserve diagnostics and retry state after failed reboot/acceptance.
  Reuse existing locked helpers without recursively acquiring the same lock.
- Retired `smoke-ssh.sh`; its caller and AGENTS runtime instructions now use
  the authoritative Rust command.

Exit: tests show ownership loss before/after a remote operation prevents later
operations; the run lock excludes competing mutations; reboot failure leaves
the run non-ready; successful smoke uses recorded host-key verification and
the selected instance. A named VM passes fixture checks and reboot through the
Rust owner. No remaining smoke wrapper owns SSH/reboot/wait sequencing.

## 2. Guard binary update delivery

Inspect `skillet/crates/cli/src/vm.rs` (`update`) and
`crates/vm/src/{delivery,transport}.rs`.

- Retain the existing run lock and build-result artifact selection. Wrap update
  delivery in the shared ownership-checked transport after the build.
- Revalidate ownership before saving deployed hashes. Preserve captured hashes
  and the previous deployment record if transfer or ownership validation fails.
- Add a regression scenario where ownership disappears between delivery calls:
  no subsequent upload/install occurs and success is not recorded. Also cover
  a partial transfer followed by a successful retry.

Exit: all update guest operations use the same checks as readiness/provisioning;
successful delivery verifies installed bytes/metadata before recording hashes.
A retained disposable VM can update and rerun smoke with the new hashes.

## 3. Make storage and service fakes honor their contracts

Inspect `skillet/crates/core/src/test_utils.rs`, storage/service traits and
real adapters, their contract tests, and host/storage/btrbk tests.

- Model directories, Btrfs subvolumes, and mounted subvolumes distinctly. Require
  existence and the requested backing mount/subvolume root; ordinary directories
  must not satisfy subvolume requirements. Expose explicit fixture setup instead
  of default success. Update host fixtures to declare their storage prerequisites.
- Preserve directory metadata and existing failure injection. Make unsupported
  fake operations return an explicit error.
- Reload must preserve a running service's active state. Inspect the real
  adapter's inactive/missing-service behavior and reproduce it in the fake;
  reload is not an implicit start.
- Add meaningful contract checks for missing paths, wrong resource type,
  mismatched mount/root, repeated convergence, failed effects, and reload state.
  Ensure recorder-wrapped fakes preserve those results. Do not mutate the
  workstation's mounts or services to exercise contracts.

Exit: full injected host composition still passes with explicit valid fixtures;
invalid storage fails; btrbk cannot accept a plain directory as its snapshot
source; a successful reload does not falsely make a running service inactive.

## 4. Execute bootstrap regressions in CI

Inspect `../../.github/workflows/ci.yml`, `butane/tests/bootstrap.sh`, and the
workflow's existing input filters and shell checks.

- Add an explicit step running `bash ublue/butane/tests/bootstrap.sh` from the
  repository root. Keep it credential-free and independent of libvirt.
- Ensure changes to bootstrap scripts, included units/configuration, test
  fixtures, and the workflow select the job. Retain lint coverage for executable
  helpers regardless of extension.
- Run the same bootstrap command locally and update the exact required-check
  commands in AGENTS if necessary.

Exit: CI executes the state regressions, rather than merely linting them, and
shared bootstrap input changes trigger that coverage. Record local evidence;
remote CI remains unverified until a run is observed.

## 5. Add actual host acceptance selected by capabilities

Inspect `skillet/crates/host-profiles`, host composition, service declarations,
the new Rust smoke owner, and `integration_tests/smoke-guest.sh`.

- Keep synthetic fixture acceptance and actual application acceptance explicit
  in commands/output. Define reusable acceptance expectations from canonical
  profile/service declarations; do not hard-code one host or duplicate its
  recipe configuration in an unrelated test registry.
- Check declared systemd/container services, mounts and ownership, container
  network mode, published listeners, and applicable application health. Use
  focused guest commands or a standalone guest assertion helper, invoked only
  through the guarded Rust transport. Redact credentials and private UI names.
- Compare managed bytes/metadata and relevant runtime identity around repeat
  apply. After reboot, check the declared services and persistent application
  data again. Exclude logs, caches, and counters from equality comparisons.
- Make credential-dependent application checks an explicit provisioned mode.
  Missing prerequisites fail that mode clearly; fixture success must not imply
  application success. External ACME/enrollment checks stay opt-in.
- Unit-test selection with differing/synthetic capability sets and injected
  observations. Cover stopped services, wrong mounts/network/listeners, changed
  managed state, and lost persistence without live providers in routine CI.

Exit: a named provisioned clamps VM passes checks for its declared services,
repeat apply, and reboot persistence. A second profile/capability combination
proves selection is generic in routine tests. Report exactly which services
were checked and which opt-in checks were omitted. Keep production renewal,
outside-tailnet access, and other previously deferred checks deferred.

## 6. Reconcile documentation and close the gate

- Update the roadmap and affected slice progress to reflect implemented versus
  pending work. Reconcile `design/skillet-architecture.md`, `design/smoke-vms.md`,
  secrets/private-UI acceptance statements, and migration entry points.
- Correct recording format version documentation against emitted format 2,
  manifest-bound smoke override instructions, vault requirements, and native
  libvirt runtime instructions. Remove manual cleanup advice that bypasses
  provider journals or the Rust lifecycle owner.
- Update the crate map and record general prevention rules in AGENTS: every
  lifecycle entry point shares locking/ownership checks; fakes preserve resource
  type/state; CI executes required regression suites; fixture acceptance cannot
  substitute for declared application acceptance. Avoid incident-specific rules.
- Record exact commands, code/artifact versions, outcomes, and unavailable checks
  in `butane/ACCEPTANCE.md`. Preserve earlier evidence. Remove completed slice
  plans only after decisions/evidence and remaining tasks have durable homes;
  update links when removing them.

Exit: commands and design claims match the implementation. No required check is
described as passed without evidence. All five preceding exits pass before the
roadmap is marked complete; explicitly deferred checks remain visible.

## Final validation and handoff

Run the applicable local CI checks: workspace format, pedantic all-target
Clippy, all-target tests, the Fedora base container integration command in
AGENTS, bootstrap regressions, and workflow ShellCheck for changed helpers.
Run named-VM checks for smoke, retained update, actual host repeat apply, and
reboot. Preserve failed-run diagnostics and ownership journals for retry.

Use focused commits, do not push without authorization, and report the completed
items, interface changes, validation evidence, and any remaining blockers.
