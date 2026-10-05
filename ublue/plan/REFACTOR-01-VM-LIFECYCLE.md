# 1. One VM lifecycle owner; migrate Bash management to Rust

Status: implementation and required VM acceptance complete. Fresh/retained
cycles, live legacy-manifest import, and interrupted provisioning recovery
passed. See the [roadmap](SKILLET-REFACTOR.md) and 2026-10-05 acceptance entry.

## Progress, 2026-10-03

Implemented foundation: `skillet_vm` owns validated host/instance identity,
backend/connection and SSH types, version-1 `vm.json`, atomic mode-0600 writes,
and explicit legacy import. Reads do not rewrite retained runs. Legacy files,
captured/deployed hashes and provider journals remain intact. The generic CLI
uses the shared identity validator. Manifest regressions cover strict import,
preserved journals/hashes, permissions, ownership, and path refusal.

Rust now owns status inspection and disposal through `VmBackend`, a bounded
native/Flatpak libvirt adapter, and one locked cleanup orchestrator. Both
public destroy paths use it. Successful lists distinguish an absent guest
from a failed connection; UUID and disk ownership are checked before cleanup
and again after external calls. Failed cleanup retains its manifest/journals.
Rust also owns template/run listing. Shared Cargo JSON artifact discovery is
implemented and used by the routine container runner; a custom target-directory
integration check selects the reported binary. `GuestTransport`/`SshTransport`
now provide explicit target/key policy, literal remote executable arguments,
noninteractive execution and upload. Subprocess stdin/output stay in memory,
with bounded concurrent I/O and zeroized copied stdin. Seventy-five VM tests
pass. Port preflight tests inject the bind result so test success does not
depend on a free port in the reserved workstation range.
No live VM was changed or imported.

Retained binary updates now use Rust's shared lock, ownership checks, Cargo
artifact discovery and SSH transport. Both installed artifacts are verified
before deployed hashes are saved; original capture evidence is preserved.
Readiness now uses Rust's shared transport and lock, bounded SSH/signed-boot
probes, captured artifact verification, explicit profile boot expectations,
base/user-environment checks and mode-0600 diagnostics. The CLI, create path,
`test-vm ready` and the RUN_DIR compatibility helper share that implementation.
Identity for the compatibility helper comes from validated recorded fields.
Readiness first persists `Started`, invalidating any earlier success; it saves
`Ready` and deployed hashes only after acceptance and a final ownership check.
Every guest command and upload checks ownership before and after the operation,
so a change during a long service start stops subsequent guest work.
Rust now selects and prepares native or Flatpak libvirt, resolves the FCOS
image, builds its two guest artifacts, prepares the disk/key, stages and
compiles Butane through the standalone compiler, defines/starts the guest, and
runs readiness from one create command. Matching interrupted runs resume from
the manifest. The shell `test-vm` interface only translates arguments; the
independent Bash installer and hidden CLI bridge commands are retired. No
remaining script owns VM creation, reboot, access, inspection, or disposal.
Workstream 4 moved production and disposable-VM guest commands onto the shared
verified SSH transport. Boot expectations are resolved by the canonical host
profile. Workstation provisioning state owns provider cleanup journals
separately from the VM manifest and local artifact ownership. Fresh and
retained VM create/readiness/provision/smoke/reboot/cleanup checks passed. See
[creation validation](../butane/ACCEPTANCE.md#recoverable-vm-creation-intent-2026-10-03)
and [native XML validation](../butane/ACCEPTANCE.md#native-xml-rendering-2026-10-03),
plus [readiness validation](../butane/ACCEPTANCE.md#readiness-retry-recovery-2026-10-03).

Progress, 2026-10-05: on the retained clamps VM, temporarily withheld `vm.json`
and ran `test vm ready clamps smoke --verbose`. Skillet imported the legacy
`run.conf`, validated the recorded UUID and disk against the live domain,
reapplied readiness, and wrote a mode-0600 versioned manifest. The legacy file
and its SHA-256 remained unchanged.

On a fresh `clamps-test-recovery` VM, interrupted first-time provisioning after
the mode-0600 Tailscale pending journal was written but before a completed
device record existed. The journal and owned VM identity remained intact after
Ctrl-C. Retrying provisioning wrote the device record and removed the pending
marker; fixture smoke/reboot passed, and destroy removed the journaled Tailscale
device and VM artifacts. See the 2026-10-05 acceptance entry.

## Read and locate

- `../design/{skillet-architecture,smoke-vms,secrets,cloudflare-ui-lifecycle}.md`.
- `../butane/bin/{test-vm,test-vm-ready,butane,flatpak-virt}` and
  the `virsh`/`virt-install` wrappers; `../butane/README.md`.
- `../skillet/crates/cli/src/main.rs` VM dispatch and
  `secret_delivery.rs` provisioning, external ownership, cleanup, and VM SSH.

Review findings: disposal used to bypass external cleanup or require a live
guest; those paths are fixed. Persisted intent makes interrupted creation
inspectable, and the canonical host profile supplies readiness expectations.
Fresh creation, readiness, interrupted retry, and cleanup have passed live.

## Implementation sequence

1. Add workstation library `skillet_vm` with `thiserror` errors and thin CLI
   delegation. Define `VmRun`, a versioned manifest, backend/connection and
   SSH target types. Keep host profile, instance, guest hostname, environment,
   UUID, disk, SSH port/key/known-hosts, source and deployed artifact hashes
   distinct. Use atomic manifest updates with restricted local permissions.
2. Import existing `run.conf`, UUID/hash files, and external ownership metadata
   without deleting or recreating retained VMs. Validate paths, identities, and
   supported legacy formats before conversion. Reject ambiguous state with a
   useful diagnostic; missing evidence is not authorization to adopt a domain.
3. Define `VmBackend` for inspect/define/start/reboot/stop/undefine and
   `GuestTransport` for execution and file delivery. Initially implement native
   and Flatpak subprocess adapters invoking focused tools directly. Preserve
   current connection/runtime selection and networking behavior. Do not link
   native libvirt, embed shell, or install tools during this migration.
4. Move list/status/create/reboot/update into Rust. Native domain XML now uses
   `quick-xml`; Flatpak uses a focused `virt-install` adapter. Rust owns atomic
   disk preparation, per-run key creation/validation, structured Butane
   staging, backend definition/start, tool-version capture, and domain UUID
   bookkeeping. Rust invokes the standalone Butane compiler. Build
   artifacts through a shared Cargo helper. Validate required
   tools and SSH-port conflicts before costly provisioning. See
   [native XML validation](../butane/ACCEPTANCE.md#native-xml-rendering-2026-10-03).
5. Persist creation intent and a generated UUID before domain definition;
   advance through explicit recoverable phases as side effects complete.
   Failed starts retain owned state for inspect/retry/disposal. Distinguish
   an absent domain from an inaccessible backend. Validate actual UUID/disks
   whenever a domain exists; refuse mismatches and unrelated paths.
6. Move readiness into Rust with bounded waits, progress, and retained guest
   diagnostics. Use the selected profile and manifest, never a fixed host.
   Preserve signed bootstrap, SELinux/resolver and user-environment checks,
   base-apply verification, and captured-artifact versus update semantics.
   Workstream 4 will consolidate its transport with credential delivery.
7. Make one lifecycle orchestrator own destruction. Supply external cleanup
   through a narrow callback/interface backed initially by existing provider
   code. Validate recorded ownership independently of domain presence; clean
   external resources first, then verified local resources, then metadata.
   Persist completed cleanup phases for retry. Retain metadata when cleanup
   fails or a response is ambiguous; never treat a connection error as absence.
8. Route every public entry point through this orchestrator. Retire Bash VM
   management or retain compatibility wrappers that only forward to Rust,
   including destruction. Update READMEs and the smoke VM design with actual
   implemented behavior. Guest smoke assertions can remain standalone Bash.

## Validation and exit criteria

- CI tests with fake backend/transport/cleanup cover successful transitions,
  timeout diagnostics, port conflicts, failed definition/start, existing-name
  refusal, UUID/disk mismatch, legacy import, missing domains, and cleanup
  retries. Assert that failure preserves recovery state and unrelated resources.
- Native and Flatpak adapter contract tests verify arguments, environments,
  connection selection and XML/network differences without starting real VMs.
- On an available supported backend run a named fresh VM through create,
  ready, update, reboot/ready and destroy. Inspect a retained legacy run before
  and after import, confirming its identity, SSH key, and original artifacts.
- Exercise cleanup after an owned domain is already absent; an injected
  external cleanup failure must retain metadata and succeed on retry. Real
  external cleanup uses only authorized disposable resources and vault access;
  unavailable backend/API checks are explicitly unverified.
- No Bash script remains an independent VM lifecycle owner or provides local
  destruction that discards pending external ownership. Shared shell assertion
  or compiler fixtures are allowed and keep their explicit interfaces.
- Pass the roadmap checks, record live evidence in `../butane/ACCEPTANCE.md`,
  and update designs, usage, and general AGENTS lifecycle rules before handoff.
