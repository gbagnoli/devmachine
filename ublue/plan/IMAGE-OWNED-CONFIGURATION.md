# Image-owned Skillet configuration and scheduled reboots

Status: in progress, 2026-11-06. Staged kexec activation has native VM acceptance;
production scheduling remains disabled. The ownership inventory and public binary
bundle exporter are implemented. Exact-revision image CI integration passed hosted
builds; the initial binary bundle booted on Clamps. The btrbk vendor pilot and
recoverable migration passed scoped native retained-VM acceptance. Fresh Ignition,
whole-host rollout and the signed pilot image build/boot remain pending.
Existing mutable runtime owners are retained until explicit migration acceptance.

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

Initial audit: [ownership inventory](IMAGE-OWNERSHIP-INVENTORY.md). Clamps backup
acceptance remains in its tracker; artifact-only work does not change production.

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

Implemented entry point: `skillet image <host>`; [bundle guide](../skillet/crates/workstation/docs/image-bundles.md).
Current allowlist is matching binaries, a public release manifest and caller-selected
btrbk vendor artifacts. General retained-state executable migration remains pending.

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

Implemented: pure shared renderer and bundle inventory, image Rust guard, exact
legacy-content/receipt preflight, recoverable regular-file retirement, timer
enablement migration and explicit legacy fallback on older images. Keep a
migration-capable mutable caller for rollback reconciliation. CI tests cover
interrupted retirement/activation, edits, missing vendor files and repeated apply.
Native `clamps-test-kexec` acceptance passed: absent module state on the vendor
image, upgrade with retained definitions, custom-edit refusal, a real activating
oneshot, recovery from a retained partial-retirement fixture, no-op repeats, actual
snapshot/restore, normal reboot and OS rollback with module reconciliation.
Synthetic data and encrypted credential hashes stayed unchanged. Local OCI builds
used explicit unsigned import; Secure Boot/TPM remained enabled. This is not fresh
Ignition or full application-profile acceptance. The published signed pilot image
and production transition remain unverified. See [evidence](archive/ACCEPTANCE.md).

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

## Slice 5 — daily reboot policy with verified kexec activation

The operator's eventual policy is a daily reboot, including days without a new
OS deployment. The operator prefers moving directly to kexec if accepted. The
VM mechanism below now passes; implement one scheduler with explicit activation
methods rather than a normal-only scheduler followed by a second redesign. Keep
scheduling disabled until production acceptance and a supervised Clamps handoff pass.

- Use one reboot owner. Retain the effective uCore update-staging mechanism;
  inspect deployed versions rather than enabling competing Zincati/rpm-ostree
  schedulers. Staging downloads/prepares updates; scheduling chooses activation.
  Do not finalize or load a kernel merely because an image was staged. At each
  eligible reboot window, re-read the current pending deployment and prepare that
  target; if none is pending, prepare the running deployment. A deferred reboot
  leaves updates staged so a later update may replace the target before the next
  window. Serialize target selection against concurrent staging.
  `libglnx` uses open-file-description locks when available and unlinks its
  lock file on release; a plain `flock` on the same path does not coordinate
  with it on Linux. The activation implementation must prove interoperability
  with the installed OSTree version, including lock-file replacement, rather
  than assuming that matching the path is sufficient. OSTree's own sysroot
  lock is non-reentrant, so do not hold it across `finalize-staged`.
  See [libglnx's lock-file implementation](https://github.com/GNOME/libglnx/blob/master/glnx-lockfile.c).
- Configure a per-host daily window, explicit timezone and fleet staggering.
  Agree the window/outage budget before production activation. Do not reboot both
  DNS peers together; a missing second peer requires an accepted outage policy.
- Honor maintenance inhibits and serialize with updates, enrollment, backup/restore
  and credential activation. Deferral is visible and bounded, not a silent permanent
  veto. Never force a reboot through an active recovery/restore operation.
- Prevent catch-up reboots immediately after startup and repeated reboot loops.
  Missed windows defer to the next eligible window; track the attempted window.
- For accepted kexec hosts, explicitly finalize/select the approved deployment and
  load its matching signed kernel/initramfs/options before graceful kexec shutdown.
  Quiesce configuration writers and serialize activation: the VM experiment finalized
  live in isolated state; production must preserve late `/etc` changes and stop on
  finalization/load errors before requesting shutdown. Never select an old default
  entry merely because the new deployment is not yet finalized. Load only
  immediately before the intended handoff; if activation is abandoned, unload the
  prepared kernel and verify it is no longer armed before returning. Systemd may
  otherwise use it for a later ordinary reboot. An unload failure is visible and
  blocks further automatic reboot attempts until resolved.
- Keep an explicit full firmware reboot method for firmware work, recovery and
  periodic complete boot checks. A monthly full reboot is a proposed starting
  cadence, not an established requirement; choose a per-host schedule after the
  physical trial and stagger DNS peers. Any pre-handoff fallback policy must be visible;
  a hung kernel handoff cannot promise software fallback. Firmware, dbx/BIOS and
  TPM policy mutations remain separate supervised operations.
- Expose last attempt/success, deferred reason, booted release, required container
  digests and post-boot health through module-owned monitoring. A system reaching
  SSH alone does not establish Pi-hole/backup/monitor recovery.

Exit: timer policy tests plus named UEFI/TPM VM tests for no-update daily reboot,
staged userspace and kernel updates, inhibitors/deferrals, missed windows,
interrupted staging, a replacement update before the window, abandoned-load
cleanup and reboot-loop prevention. Verify boot ID, deployed digest,
`uname -r` when the release changes kernel, automatic unlock, mounts, private UIs,
DNS, credentials, backup schedules and monitoring. Rehearse explicit rollback with
compatible data. Then one supervised physical Clamps rollout, followed by observed
daily runs. Firmware installation stays manual; recovery/password policy is unchanged.

Implemented staged activation: the installed service takes cooperative configuration
exclusion. The `skillet_kexec` crate owns the renderer, convergence and runtime;
Clamps opts in through its canonical service declaration. Base and full apply
install/update the unit without starting activation or enabling a timer. The
separate `kexec install` command is removed. Interrupted systemd reloads are retried
using a successfully-applied definition marker. The service takes configuration
exclusion, checks the fresh target under native OSTree OFD locking, pauses the
existing rpm-ostreed staging owner and finalizes the selected pending deployment.
Its private mount namespace permits writable sysroot operations without changing
the host mount. It loads the exact checksum/serial's matching BLS artifacts.
Post-finalization failures retain a boot-local gate that refuses further managed
configuration writes until reboot; cleanup unloads its kernel and restores its
runtime staging mask. Other root tools must respect the maintenance boundary.
A remote apply interrupted between phases is retried after reboot, rather than
being an atomic transaction. Timer policy, physical acceptance and the remaining
broader service-health matrix are still pending.

2026-11-05 VM acceptance: the Rust probe now has a disposable-test-only
`--load-and-hold` mode. `butane/tests/kexec-interruption.py` runs it under
systemd supervision with `ExecStopPost=kexec -s -u`. Killing the armed worker
with SIGKILL cleared `/sys/kernel/kexec_loaded`. The same unit with
`DefaultDependencies=no` survived the normal shutdown transaction and permitted
a real same-kernel `systemctl kexec`; the guest returned with a changed boot ID,
Secure Boot/integrity lockdown, automatic PCR7 unlock, the expected deployment,
and verified synthetic encrypted credential/data fixture. The journal records
`System is rebooting with kexec` and successful `systemd-kexec.service`.
No staged-update coordination or unattended production owner is established by
this fixture. The transient test service has no production scheduling role.

Subsequent 2026-11-05 service acceptance: installed the host binary and its actual
`skillet-kexec-activate.service` in the owned VM. Service start completed a graceful
same-kernel handoff with a changed boot ID. The final worker was also tested with
a blocking shutdown inhibitor: handoff failed, the kernel was unloaded and its
ownership marker removed. A SIGSTOP while the loader child ran allowed SIGKILL
of the actual worker after arming; `ExecStopPost` again unloaded the kernel and
removed the matching ownership marker. No physical host was changed.

Forced-poweroff recovery: after the service handoff, `virsh destroy` followed by
`virsh start` performed an abrupt power loss and normal UEFI boot. The guest
returned unattended with Secure Boot, integrity lockdown, PCR7 root binding,
Btrfs root, expected deployment, unchanged synthetic data and a successfully
decrypted synthetic credential; no kernel remained armed. This does not establish
staged-update recovery or application database durability under power loss.

## Kexec mechanism — native VM accepted, physical acceptance pending

Physical gate remains open after 2026-11-06 testing. An initial supervised Clamps
handoff succeeded, but the next staged activation stopped responding after the
old boot completed its kexec shutdown transaction. No new boot journal or pstore
record explains the failure. Operator power cycling recovered the intended signed
image and cleared the boot-local gates. This is not reliable unattended kexec
acceptance. Keep scheduling disabled; further physical testing needs a working
console and early-boot evidence to distinguish late shutdown, handoff, initramfs
unlock and device recovery. Do not infer handoff completion from a successful
`systemd-kexec.service` shutdown job alone.

Research checked 2026-11-02. Linux kexec can boot a different kernel, whereas
systemd soft-reboot keeps the current kernel. Thus kernel updates are possible
in principle with kexec, but userspace soft-reboot does not apply them.

### Physical Clamps investigation — 2026-11-02

Read-only SSH inspection confirms the deployed uCore has kernel
`7.2.9-201.fc44.x86_64`, systemd `259.9`, kexec-tools `2.0.32`, OSTree `2026.4`
and rpm-ostree `2026.3`. Kernel configuration enables `KEXEC`, `KEXEC_FILE`,
`KEXEC_SIG` and bzImage signature verification. Kexec is not disabled by sysctl;
no kernel is loaded for kexec. Secure Boot is enabled and lockdown is in integrity
mode. This establishes available kernel/tooling capability, not a successful
signed load, kernel handoff, TPM unseal or hardware recovery.

Two concrete integration problems prevent a direct substitution of
`systemctl kexec` for ordinary update reboots:

- A deployment is currently staged, while neither existing `/boot/loader/entries`
  file resolves to it. The active `ostree-finalize-staged.service` performs its
  finalization in `ExecStop`. Systemd v259's `load_kexec_kernel()` selects/loads
  the default boot entry before initiating shutdown. Therefore selecting current
  entries before finalization cannot select this staged deployment. This is an
  ordering problem, not evidence that Linux cannot run the newer kernel.
- The ESP is not mounted and `/run/boot-loader-entries` is absent. Unprivileged
  `bootctl` discovery failed to find an ESP; explicitly pointing it at `/boot/efi`
  also failed its FAT/ESP check. `/boot` is ext4 on an XBOOTLDR-type partition.
  Systemd v259's automatic loader searches for an ESP before XBOOTLDR; this layout
  needs explicit handling or a verified compatible discovery arrangement. Do not
  report an unprivileged inspection as execution of privileged `systemctl kexec`.

Initial conclusion: the existing production staged-update path is not a direct
`systemctl kexec` substitution. The subsequent native VM trial below proves explicit
finalization and signed target loading works. Production activation still needs
its own serialized/quiesced implementation and supervised hardware acceptance.
Do not blindly preload the running entry.

Production sudo requires a password and `sudo -n` was unavailable in this
inspection. No password was requested, no kernel loaded/unloaded, no boot files,
mounts, updates, TPM/firmware state or reboot policy changed. Loading a kexec kernel
on production merely to test signatures is not read-only: it can change how a later
ordinary reboot behaves. No end-to-end kexec success is claimed.

Current upstream uCore enables rpm-ostreed staging and disables Zincati. Source
inspection matches the deployed finalization concern; no uCore-specific tested
fast-update integration was found. A separate Fedora CoreOS N5105/Coreboot user
report describes successful kernel loading followed by handoff hangs. That report
is not evidence of failure on Clamps' different N150/AMI hardware; it reinforces
why successful loading alone is insufficient.

### Completed native VM trial — 2026-11-02

Dedicated `clamps-test-kexec`, native user-session libvirt, q35 UEFI Secure Boot,
independent swtpm, LUKS2/Btrfs root and Clevis SHA256 PCR 7. No vault/provider or
production configuration was delivered. Baseline signed Clamps image booted
kernel `7.2.9-201.fc44.x86_64`.

Passed actual operations:

- File-syscall loading rejects a tampered signed kernel and accepts the original
  with Secure Boot and integrity lockdown enabled.
- Same-kernel graceful kexec returns with a changed boot ID and automatic root unlock.
- Explicitly finalized/selected signed upstream release `stable-20261028` boots
  kernel `7.2.7-200.fc44.x86_64` through kexec.
- Forward signed release `stable-20261030` boots kernel `7.2.9-201.fc44.x86_64`
  through kexec. Running kernel and booted OSTree checksum match the intended target.
- Explicit rollback via kexec returns to `7.2.7`; a subsequent full firmware reboot
  also unlocks unattended. Secure Boot/lockdown and the PCR 7 binding remain enabled.
- Durable synthetic data hashes match throughout. A synthetic host+TPM encrypted
  credential decrypts after transitions and a real `LoadCredentialEncrypted=`
  oneshot consumer verifies its hash after different-kernel, forward, rollback and
  full-reboot checks. No password or TPM-policy weakening was needed.

Both images were signature-enforced and pinned to registry digests. Private script,
command logs, final state and boot journals remain under the owned run's
`kexec-acceptance/`. See [dated acceptance](archive/ACCEPTANCE.md) for references.
The test VM is not standard host readiness/application acceptance; bootstrap and
apply were fenced to prevent re-entry into the moving Clamps image channel.

This proves the OS/kernel/TPM mechanism in this native VM. It does not prove
physical NIC/Wi-Fi/USB/UPS/Bluetooth recovery, production application health,
automatic scheduling/inhibitors, a safe unattended failure fallback, or a measured
reboot-time improvement. Keep the remaining matrix below for those boundaries.

Remaining implementation/acceptance must establish:

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
5. Preserve the passed native UEFI/swtpm matrix above; repeat affected cases when
   the production activation implementation changes. Then run supervised physical
   trials and measure downtime. VM results do not establish physical device recovery
   or Flatpak equivalence. Reserve periodic full boots and use full reboot for
   firmware work.

Exit: explicit per-host kexec activation after the remaining matrix and supervised
hardware trial pass with console recovery. The VM checks above need repeating only
when implementation changes affect them. Keep current production reboot behavior
until then; this does not block image rendering/pinning work.

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
- [Systemd v259 kernel loading before shutdown](https://github.com/systemd/systemd/blob/v259/src/systemctl/systemctl-start-special.c)
- [Systemd v259 ESP/XBOOTLDR discovery](https://github.com/systemd/systemd/blob/v259/src/shared/bootspec.c)
- [FCOS hardware-specific kexec report](https://github.com/coreos/fedora-coreos-tracker/issues/2071)
- [systemctl kexec and soft-reboot](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html)
- [Linux kexec signature/lockdown validation](https://github.com/torvalds/linux/blob/master/kernel/kexec_file.c)
- [Shim TPM measurements](https://github.com/rhboot/shim/blob/main/README.tpm)
