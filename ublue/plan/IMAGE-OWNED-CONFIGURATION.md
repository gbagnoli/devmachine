# Image-owned Skillet configuration and scheduled reboots

Status: planned, 2026-11-02. Finish [Clamps production acceptance](CLAMPS-MIGRATION.md)
first. This is the next architectural workstream, not a reason to interrupt the
current production rollout. No image pins, runtime ownership, auto-update policy,
reboot scheduler or kexec behavior are changed by this plan.

## Goal and boundaries

Make a signed host image identify its Skillet implementation, static service
configuration and pinned application images. Reduce guest Skillet to operations
that need the actual machine, private inputs or application state. Keep the
canonical host declarations and pure renderers shared between build and runtime;
do not execute today's complete guest apply against an image build container.

Image-owned: generic/caller binaries and helpers, service selection, systemd units,
Quadlets, static configuration, security policy, directory declarations, backup
schedules and collector programs. Runtime-owned: credentials, private environment
configuration, enrollment/provider APIs, actual mounts/subvolumes/ownership,
first-use application initialization, backups/restores and application-managed data.
Private hostnames, addresses, zones and credentials stay out of public images,
rendered CI artifacts and logs. Image builds never unlock a vault or contact a
production host/provider account.

Read `../AGENTS.md`, `../skillet/AGENTS.md`, the image repo's instructions, and
[architecture](../design/skillet-architecture.md),
[image ownership](../design/image-owned-configuration.md),
[host application](../design/host-application.md),
[secrets](../design/secrets.md), [storage](../design/storage.md),
[Podman](../design/podman-runtime.md) and [TPM acceptance](TPM-ENCRYPTED-ROOT.md).

## Slice 0 — production baseline and ownership inventory

Start implementation after Clamps production handover/backups are accepted in the
Clamps tracker. Park optional/backend checks explicitly rather than declaring all
historical tests blockers. Save current service/data/credential acceptance before
changing ownership; do not reinitialize an installed service to test this work.

Inventory each declared output/effect and classify it as image content, private
runtime input, host preparation, first-use initialization or application state.
Include all unit executable paths, writable binary overrides, receipts, Quadlets,
config paths, security policy, image builds/transfers and auto-update labels.
Identify consumers that lack a vendor-config path or need a private overlay.

Exit: reviewed ownership table, first pilot and retained-state migration contract;
no ambiguous dual owner or build-time need for live systemd, Btrfs or credentials.

## Slice 1 — pure build output and immutable guest artifacts

1. Extract/reuse pure declarations and renderers; avoid a second host/service
   inventory or executing apply through mocks to capture filesystem side effects.
2. Add an explicit image-render entry point with a selected canonical host and
   output root. Exact CLI is chosen during implementation and documented then.
   Its allowed output is deterministic, public and explicitly enumerated.
3. Install matching generic/caller binaries into image-owned `/usr/bin`, helpers
   under an appropriate `/usr/libexec` owner. Audit actual call sites before
   choosing final paths. Carry source revision and component manifest in the image.
4. Make ucore-images consume one verified, exact devmachine source revision or
   published build artifact. Do not fetch a moving branch at build/boot or trust
   an unverified binary download. Preserve image signing and provenance.
5. Migrate the existing mutable `/var/usrlocal/bin` executable/unit references
   explicitly. Ensure precedence cannot keep running an older binary and that
   retained Ignition inputs and the previous boot deployment remain usable.

Exit: deterministic render tests; image build succeeds without a vault, host
access or service activation; generic/caller versions match the recorded revision.
An owned VM boots and runs the image's artifacts without workstation deployment.
No production deployment in this slice.

## Slice 2 — one static service pilot and ownership migration

Use caller-selected btrbk as the first candidate: configuration/timer are static,
while mount/source checks and actual snapshots remain runtime operations. Confirm
this choice against slice 0; keep its explicit leaf source, never snapshot all data.

- Prefer consumer vendor locations in `/usr`; Quadlets use
  `/usr/share/containers/systemd`, systemd units `/usr/lib/systemd/system`.
  Use native tmpfiles/sysusers declarations only where they express the required
  behavior; actual Btrfs creation and observed identities remain runtime-owned.
- Inventory legacy `/etc` files and mutable executables that would shadow image
  content. Migrate only verified Skillet-owned content, with receipts/recovery.
  Preserve unrelated admin overrides and application edits; fail with actionable
  guidance on conflicting ownership instead of silently removing them.
- Update boot ordering so runtime data/credential preparation precedes consumers.
  Repeated boot preparation must be idempotent and need no workstation/vault.
- Keep legacy runtime mode explicit during the transition. Image-owned mode must
  not write a second copy under `/etc` on each apply. Normal operation has one owner.

Exit: fresh VM, upgrade from retained mutable state, repeat boot, interrupted
migration, explicit override/conflict and OS rollback pass. New image defaults
become effective; unchanged application data/credentials and old rollback behavior
are verified. Directory preparation still works on a fresh installation.

## Slice 3 — pinned application images and release inputs

- Introduce a reviewed lock manifest covering every selected container image,
  including custom Caddy/cache/helper images. Use registry digests for immutable
  selection, with readable upstream version/channel and architecture metadata.
  Digests, not mutable tags alone, determine the deployed bytes.
- Resolve pins before rendering; share them between production, VM acceptance and
  host composition. Test overrides are explicit and cannot silently affect prod.
- Pin upstream uCore and build dependencies where supported. Record actual inputs
  and resolved component digests; pinning does not claim bit-identical RPM builds
  without controlled package inputs. Preserve common-before-host CI ordering;
  host builds must use that run's exact common digest, not race on `:latest`.
- Keep a moving signed host release channel for normal staged fleet upgrades.
  Published releases record exact OS/component identities and retain rollback
  references; deployment channels and immutable release inputs are distinct.
- Add an automated update proposal workflow: resolve new digests, show version
  changes, validate/build/test, then promote the approved signed release. Daily
  rebuilds of unchanged pins alone must not be described as software updates.
- Remove independent registry auto-update labels for image-owned workloads when
  they adopt pins. Preserve unrelated explicitly managed consumers/timer policy;
  do not globally disable another owner's updates as a side effect.
- Arrange selected container-image availability before the reboot window. Avoid
  making service recovery depend on a slow registry download during startup;
  document whether images are pre-fetched or bundled and test the chosen policy.

Exit: CI rejects unresolved image references; all selected containers run expected
pinned digests after boot; runtime auto-update cannot advance them independently.
A tested update proposal changes pins and the release manifest coherently. Cached
startup works without registry access, with a bounded failure when required bytes
are absent. OS rollback does not imply data/database rollback.

## Slice 4 — expand ownership and shrink ordinary apply

Move remaining services incrementally, starting with public static definitions.
Keep the same canonical declarations, credential names and prerequisite ordering.
Private domains/addresses and secret-derived configurations remain encrypted
runtime inputs or volatile overlays; do not push them into public CI to increase
image coverage. Separate private configuration from stable service structure.

Normal workstation apply eventually delivers/reconciles private inputs and
activates only their declared consumers; ordinary code/static changes arrive via
the signed image. Live binary/config repair remains an explicit diagnostic path
with reported drift and a reviewed return to image ownership. Do not remove it
until image rollout and rescue are accepted. Application edits, databases and
identity keys remain outside image convergence.

Exit per service: no duplicate writer, fresh boot and retained-state upgrade,
credential rotation, no-op repeat, restart/reboot, data preservation and safe
failure tests pass. Report image revision, private configuration revision and
actual component digests separately without leaking values.

## Slice 5 — daily normal reboot policy

The operator's eventual policy is a daily reboot, including days without a new
OS deployment. First implement ordinary firmware/bootloader reboots. Keep them
disabled until production acceptance and the VM/supervised rollout below pass.

- Use one reboot owner. Retain the effective uCore update-staging mechanism;
  inspect deployed versions rather than enabling competing Zincati/rpm-ostree
  schedulers. Staging downloads/prepares updates; scheduling chooses activation.
- Configure a per-host daily window, explicit timezone and fleet staggering.
  Agree the window/outage budget before production activation. Do not reboot both
  DNS peers together; a missing second peer requires an accepted outage policy.
- Honor maintenance inhibits and serialize with updates, enrollment, backup/restore
  and credential activation. Deferral is visible and bounded, not a silent permanent
  veto. Never force a reboot through an active recovery/restore operation.
- Prevent catch-up reboots immediately after startup and repeated reboot loops.
  Missed windows defer to the next eligible window; track the attempted window.
- Use graceful shutdown and normal OSTree staged-deployment finalization. Firmware,
  dbx/BIOS and TPM policy mutations remain separate supervised operations.
- Expose last attempt/success, deferred reason, booted release, required container
  digests and post-boot health through module-owned monitoring. A system reaching
  SSH alone does not establish Pi-hole/backup/monitor recovery.

Exit: timer policy tests plus named UEFI/TPM VM tests for no-update daily reboot,
staged userspace and kernel updates, inhibitors/deferrals, missed windows,
interrupted staging and reboot-loop prevention. Verify boot ID, deployed digest,
`uname -r` when the release changes kernel, automatic unlock, mounts, private UIs,
DNS, credentials, backup schedules and monitoring. Rehearse explicit rollback with
compatible data. Then one supervised physical Clamps rollout, followed by observed
daily runs. Firmware installation stays manual; recovery/password policy is unchanged.

## Separate experiment — kexec, not an initial requirement

Research checked 2026-11-02. Linux kexec can boot a different kernel, whereas
systemd soft-reboot keeps the current kernel. Thus kernel updates are possible
in principle with kexec, but userspace soft-reboot does not apply them.

Current upstream uCore build setup enables rpm-ostreed automatic staging and
disables Zincati. No supported uCore kexec update workflow was established in
this research; systemd documenting kexec is not acceptance on the deployed image.
Do not assume the newest systemd manual describes the installed implementation.

An experiment must establish:

1. Actual deployed kernel configuration, tools, systemd version, Secure Boot/
   lockdown rules, trusted signing keys and platform support. Do not weaken Secure
   Boot, disable signature checks or change TPM policy to make a fast reboot work.
2. Correct finalization and selection of the staged OSTree deployment. Its kernel,
   initramfs and command line, including `ostree=`, must agree. Staged boot entries
   can be finalized only during shutdown; loading the old default first can miss
   the intended update. Never blindly reuse the running kernel command line.
3. Accepted signed-kernel loading, encrypted-root TPM unseal and preserved recovery.
   Skipping firmware/shim changes the boot path; PCR values/event logs and driver
   behavior must be observed. Successful kexec does not prove the next real boot.
4. Normal service shutdown, storage integrity, NIC/Wi-Fi/USB/Bluetooth/UPS recovery,
   and bounded return to service. A failed transition may need console/power access;
   a local shell cannot promise fallback after it loses control.
5. Named UEFI/swtpm VM trials for same-kernel restart, real kernel update, retained
   credentials/data, wrong-signature rejection, rollback and a subsequent full
   firmware reboot. Only then supervised physical trials and measured downtime
   comparison. Reserve periodic full boots and use full reboot for firmware work.

Exit: optional explicit per-host fast-reboot capability only after the complete
matrix passes with console recovery. Otherwise retain normal daily reboots.
This experiment does not block the image-ownership or daily reboot slices.

## Validation and completion

Use required local Rust/Clippy/integration checks for each implementation slice.
CI pure/render/migration tests require no vault/provider access. Named VM effects
hold the existing lock/ownership boundaries; never embed shell programs in Rust
or install tools on the workstation. Record exact evidence and unfinished gates
in [acceptance](archive/ACCEPTANCE.md). Update designs/README ownership descriptions
as behavior changes; do not rewrite a future plan as already implemented.

Completed target: a signed release binds code, public configuration and application
image digests; runtime handles only explicit local/private/stateful effects; tested
daily reboots activate approved releases without unplanned live configuration drift.

## Research references

- [uCore build update defaults](https://github.com/ublue-os/ucore/blob/main/ucore/post-install-ucore-minimal.sh)
- [rpm-ostree staging policy](https://github.com/coreos/rpm-ostree/blob/main/man/rpm-ostreed.conf.xml)
- [OSTree staged deployment and boot arguments](https://github.com/ostreedev/ostree/blob/main/docs/deployment.md)
- [OSTree configuration merging](https://ostreedev.github.io/ostree/atomic-upgrades/)
- [Quadlet vendor locations and precedence](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
- [systemctl kexec and soft-reboot](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html)
- [Linux kexec signature/lockdown validation](https://github.com/torvalds/linux/blob/master/kernel/kexec_file.c)
- [Shim TPM measurements](https://github.com/rhboot/shim/blob/main/README.tpm)
