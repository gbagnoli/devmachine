# Skillet architecture refactoring

Decision, 2026-10-03: complete the seven refactoring workstreams before further
service migrations, encryption implementation, or production cutover. Existing
service behavior and unfinished acceptance requirements remain authoritative.
Status: planned; the review and plans do not establish runtime acceptance.

Implementation progress: `skillet_vm` now supplies validated run identity and
versioned manifests with explicit, non-destructive legacy import. Rust status
and disposal use a focused libvirt adapter and a locked cleanup orchestrator;
listing is also in Rust. Shared Cargo JSON artifact discovery selects the exact
reported binary and now serves the container runner and retained VM updates.
Updates share the ownership lock and SSH transport, preserve captured hashes,
and record deployed hashes only after verifying both installed artifacts.
Readiness now uses the same locked owner and SSH transport, with explicit
boot expectations from the canonical host profile, bounded probes and private
diagnostics. It verifies original artifacts and signed boot evidence before
recording readiness. A retry first clears the persisted Ready checkpoint and
restores it only after acceptance. Guest operations are guarded by ownership
checks before and after each call. Production and disposable-VM credential
delivery use the same verified guest transport and typed credential installer.
Transport's bounded I/O stays in memory; payload copies are zeroized after
stdin transfer. Provider and VM lifecycle orchestration still needs to move
behind the workstation library boundary.

Workstream 2 adds `skillet_hosts` as the single host capability declaration.
It owns host composition and supplies boot, service, network, UI, storage, and
credential policy to entry points. The explicit `agent` baseline is the generic
no-service apply profile. Synthetic smoke fixture composition lives in a
separate test command and is not dispatched as a host identity. Workstation
Tailscale enrollment now accepts provider, profile/environment, VM identity,
state path, and guest transport as explicit inputs. Retained-VM
runtime acceptance remains pending.

Workstream 3 is in progress. Account observations and subordinate-ID files
enter composition through injected resource interfaces; missing or invalid
subordinate ranges fail closed. Directory ownership uses one typed named or
numeric identity contract, and entry points pass required systemd credentials
to host composition. Remaining broad file/system traits and fake contracts
are still scheduled for this workstream.

Keep application crates as reusable recipes. Separate canonical host profiles
and composition from guest runtime adapters, CLI parsing, workstation
provisioning, and synthetic test fixtures. Effects need injectable capability
boundaries covering both observations and mutations. Pure configuration and
composition use ordinary data and functions. This makes host tests independent
of the workstation and keeps one declaration authoritative across entry points.
Podman runtime identity, host data ownership, and namespace mappings remain
separate configuration concepts; see [Podman runtime configuration](podman-runtime.md).

Move VM lifecycle orchestration from Bash into Rust. One orchestrator owns the
typed manifest, identity validation, readiness, recovery, and destruction,
including external-resource cleanup before deleting local recovery metadata.
Native and Flatpak execution remain supported. Initially invoke focused
`virsh`, SSH, and existing compiler tools through subprocess adapters; native
libvirt linkage would add requirements to the static musl build and Flatpak
workstations. Keep that choice behind an interface. Do not embed shell programs
in Rust or install workstation dependencies during the migration.

Preserve command behavior and import existing run manifests before replacing
their format. Guest assertion scripts may remain standalone fixtures. Recording
is diagnostic; fast tests establish decisions and adapter contracts, while
named disposable VMs establish real service and reboot behavior.

Implementation order and exit criteria:
[refactoring roadmap](../plan/SKILLET-REFACTOR.md). Existing runtime design:
[smoke VMs](smoke-vms.md), [secrets](secrets.md), and [storage](storage.md).
