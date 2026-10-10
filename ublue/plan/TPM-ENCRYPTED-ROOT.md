# TPM-encrypted uCore root

Prerequisite: complete [the Skillet refactoring roadmap](SKILLET-REFACTOR.md)
before proceeding with the remaining feature milestones in this plan.
Existing acceptance evidence and deferred tasks remain applicable.

Status: native profile implemented 2026-10-10. Fresh signed bootstrap,
independent-key recovery/cold boot, runtime fixture, plain-profile readiness,
and owned disposal/recreation passed in native disposable VMs.
The accepted scope is encrypted-disk loss/removal with native Clevis SHA256
PCR 7 plus Secure Boot. Custom UKIs/signed PCR policies and Tang are deferred.
Unencrypted root remains supported and is the default. Physical and Flatpak
acceptance are unverified. The refactor live retry remains parked.

Implemented: typed optional manifest/install profile, conflict validation,
mapper-safe Btrfs mounts, common UEFI/TPM XML, capability preflight, owned
TPM/NVRAM disposal, private serial logging, profile-aware readiness and the
repeatable independent-key/header/cold-boot acceptance command. Routine CI
covers rendering, prerequisites, staging and policy rejection without a vault.
Production recovery-key vault persistence is implemented with atomic pending/verified
state, volume identity validation, and a separate VM archive command. Live
acceptance of this new vault workflow, production header export and physical
installation remain pending.

## Scope and decisions

Use LUKS2 beneath the shared Btrfs root backing `/sysroot`, `/var`, and the
`data` subvolume mounted at `/var/lib/data`. Keep uCore's composefs/deployment
layout, application paths, Podman graphroot, and btrbk leaf snapshots.
EFI and `/boot` remain outside encryption; boot integrity needs a separate
verified policy. Normal boots must unlock locally without an operator or
network. Every host gets an independent recovery key and LUKS header backup.

Prefer TPM2 on local and remote physical servers. Assess each remote server's
actual TPM and firmware before choosing its install profile. Tang is deferred;
do not implement a Tang service, initramfs VPN, or public unlock endpoint.
An explicitly requested encrypted profile must fail if its prerequisites are
missing, never silently install unencrypted storage or an unrestricted unlock
slot. Existing unencrypted development VMs may retain their explicit profile.

Read `AGENTS.md`, `skillet/AGENTS.md`, `design/storage.md`, `design/secrets.md`,
`design/smoke-vms.md`, `butane/clamps.bu`, `butane/includes/data-storage.bu`,
`butane/bin/{test-vm,test-vm-ready,butane}`, and the ucore-images recipes.
Make small validated changes in the order below. Never format a populated
physical disk as part of Skillet convergence; use only named disposable disks
until physical install and restore are explicitly arranged.

## Audit findings and next slice (2026-10-10)

- Read-only retained-VM inspection confirmed native Clevis/TPM/Btrfs unlock
  components in the deployed initramfs. No image/package change is needed yet.
  The exact initial cached FCOS image still needs boot-time verification.
- Native workstation q35 capabilities advertise EFI, secure loaders and TPM2
  emulation. swtpm 0.8, QEMU 10.1, libvirt 11.10 and OVMF with enrolled-key
  templates are installed. No physical TPM is exposed; virtual tests remain
  possible. Flatpak equivalence and physical hardware are unverified.
- Retained unencrypted XML selects q35 without UEFI/NVRAM or TPM.
  The new explicit profile extends the shared manifest/XML/backend/lifecycle
  owner while preserving retained BIOS manifests.
- `includes/data-storage.bu` now selects the verified filesystem UUID/label
  rather than a raw root partition; UUID/topology and mount guards remain.
- Separate GRUB kernel/initramfs entries are deployed, not an observed signed
  UKI/PCR policy pipeline. Native PCR-7-based unlock offers a narrower guarantee
  than authenticated whole-machine boot. The operator accepted this narrower
  scope; custom UKIs/signed policies are deferred. Do not choose empty PCRs or silently claim stronger protection.
- With the accepted scope, verify enrollment timing across FCOS, uCore and MOK,
  using the explicit profile and named UEFI/TPM VM. Keep the
  existing unencrypted retained smoke VM unchanged. Recovery material must be
  established before TPM replacement/tampering tests.

Evidence and policy limits: `../design/storage.md` and
`../butane/ACCEPTANCE.md`. Audit completion is not encrypted-boot acceptance.

## 1. Establish hardware, image, and boot policy

- Inventory exposed TPM2, supported PCR banks, UEFI/Secure Boot, installed
  image versions, and initramfs Clevis/cryptsetup/TPM/Btrfs support. On older
  systemd use `systemd-creds has-tpm2`; newer releases can use
  `systemd-analyze has-tpm2`. An empty device list is not proof that the
  motherboard lacks a disabled TPM. Do not install workstation packages.
- Determine the actual boot chain before selecting PCRs. Plain Clevis
  `tpm2: true` supplies no explicit PCR policy. PCR 7 represents Secure Boot
  state; it alone does not authenticate our kernel, initramfs and command line.
  Include shim/MOK handling in the assessment. Avoid choosing exact kernel or
  command-line measurements that strand unattended hosts on every update.
- Investigate signed PCR policies and their compatibility with the existing
  image build, FCOS-to-uCore bootstrap, updates and rollback. If using systemd
  enrollment rather than native Clevis, prove its initramfs unlock support.
  Record the selected mechanism, measurements, protection limits and update
  procedure in the storage design. Do not claim whole-machine theft protection
  until the boot components and alternative boot paths are covered.
- Keep packages/static initramfs support in ucore-images; host enrollment and
  private values stay out of that public image. Preserve serialized image CI.

Exit: an explicit boot policy and recovery procedure are documented, and all
bootstrap/deployed images can unlock it. Unsupported policy integration is a
reported blocker, not permission to weaken the chosen policy implicitly.

## 2. Add an explicit encrypted installation profile

- Model the selected encryption profile separately from host identity, VM
  instance and physical disk selection. Reuse it for every host that opts in;
  do not hardcode clamps into shared storage or unlock logic.
- In Butane/Ignition create LUKS2 on the selected root partition and Btrfs
  labelled `root` on `/dev/mapper/root`. Remove the existing raw-partition
  formatting entry in that profile. Keep explicit partition sizing/resizing;
  do not assume automatic growth after root reprovisioning.
- Enrollment must occur under the intended final boot policy. Account for
  MOK enrollment and the FCOS-to-uCore bootstrap before taking measurements.
  If a temporary install unlock path is necessary, document its lifetime and
  removal and prove it is absent before acceptance.
- Generalize the data helper and mount currently referencing
  `/dev/disk/by-partlabel/root`. Use a stable identifier of the unlocked Btrfs
  filesystem and verify it matches `/var` and the expected backing topology.
  Preserve wrong/missing-mount guards and SELinux labeling for Podman.

Exit: generated Ignition and units target the mapper filesystem, carry no
private key, and preserve the existing data layout. Generation rejects missing
capabilities and conflicting raw/encrypted formatting entries.

## 3. Add TPM-capable disposable VMs

- Add an opt-in TPM2/encrypted-install profile to the existing Rust CLI and
  VM helper. Document its actual command interface once implemented.
- Both native libvirt XML and Flatpak virt-install paths must use equivalent
  virtual TPM2 and UEFI/Secure Boot configuration. Check backend capabilities
  before creating a run; missing swtpm/firmware support produces a useful error.
  Use libvirt-managed swtpm, not passthrough of the workstation's physical TPM.
- Give each named VM independent TPM state and firmware NVRAM. Persist them
  across shutdown/start/rebase; own and remove them during disposal. Never
  reuse another VM's state. Record profile and ownership in ignored metadata.
- Readiness must be bounded and capture serial/early-boot diagnostics for
  unlock failures, which occur before SSH. Do not add `systemctl --wait`.

Exit: a fresh encrypted test VM reaches readiness automatically. Disposal and
fresh recreation create a new disk, TPM identity and NVRAM. Existing VM profiles
remain usable, and both backend paths have evidence or an explicit unverified
status. No workstation installation is performed by the agent.

## 4. Establish recovery before destructive tests

- Add an independent high-entropy recovery key while the valid TPM path works.
  Persist production keys through the atomic Rust KDBX writer at
  `skillet/hosts/<host>/storage/root-recovery-key`. Reuse an existing value;
  verify it unlocks this specific volume before treating it as recovery.
- Disposable tests use generated temporary recovery material, never production
  entries. Keep it out of command arguments, logs and Ignition; any temporary
  file must be ignored, mode 0600, and removed on disposal. Prefer stdin.
- Store a LUKS header backup off the encrypted disk with restricted access;
  document its private backup location and disk identity. Treat it as sensitive
  and refresh it after enrollment/rotation. Test recovery from another
  workstation using the backed-up vault. A header backup is not a data backup.
- Check for unintended key-file/unrestricted unlock slots; do not erase the
  sole working unlock path. Capture only non-secret enrollment metadata.

Exit: recovery is verified before TPM replacement or boot-policy negative
tests. TPM/motherboard loss cannot remove the only usable recovery material.

## 5. QEMU acceptance matrix

Use a fresh named disposable VM and generated secrets. Routine CI needs no
production vault, Cloudflare or Tailscale API. Networkless boot tests start
after image pulls/bootstrap; evaluate storage and base readiness separately
from services requiring the network. Record results in `butane/ACCEPTANCE.md`.

| Check | Required observation |
| --- | --- |
| Fresh install and signed bootstrap | Every bootstrap reboot unlocks without typing; final boot policy is enrolled |
| Storage topology | Raw root is LUKS2; opened mapper contains Btrfs; `/var` and `/var/lib/data` share the expected filesystem and data subvolume |
| Cold power-off/start and reboot | Same TPM/NVRAM persists; no operator input; data sentinel and test credentials survive |
| Boot with NIC disconnected | Root/data unlock and base storage readiness succeed without Tang or Tailscale |
| Signed update, rebase and rollback | Each approved deployment boots unattended and preserves sentinel, subvolumes and credentials |
| Repeat Skillet apply | Enrollment and mount bytes remain stable; no unnecessary application recreation |
| Missing/wrong data mount | Applications cannot write into a fallback directory; restoring the mount recovers without formatting |
| Disk with a different or absent TPM | Automatic unlock fails within the test deadline; no unintended key-file or weaker slot succeeds |
| Change a covered boot measurement | Unlock is denied; restoring approved state recovers; test the actual selected policy, not an unrelated PCR |
| Recovery after simulated TPM loss | Recovery key opens the same volume and preserves data; re-enrollment restores unattended boot |
| Snapshot/restore | Syncthing leaf snapshot and restore work; no whole-data snapshot or nested subvolume support is introduced |
| Disposal and recreation | Only owned disk/TPM/NVRAM/test secrets are removed; next VM receives fresh identities |

Positive-path VM tests cover functionality, not resistance to a compromised
hypervisor or physical firmware attacks. Negative tests operate on disposable
copies and never clear the workstation's TPM or change its boot firmware.

## 6. Physical acceptance and completion

- On the selected stock clamps SSD, confirm TPM2 and supported PCR bank, final
  UEFI/Secure Boot/MOK state, unattended cold boot, networkless unlock, signed
  update/rebase/rollback, storage/credential persistence, and recovery from
  external media or an available remote console. Preserve rupik and its SSD.
- Verify each later physical server independently, including remote console
  availability before enabling encryption. Do not assume a vendor/model has
  TPM support or that a QEMU pass proves its firmware behavior.
- Run applicable CI checks and affected real-runtime smoke tests after code
  changes. Keep design/README setup and VM lifecycle ownership documentation
  updated. Make focused commits only after validation; report unavailable
  checks honestly. No push without instruction.

Done when QEMU acceptance and stock-SSD physical acceptance both have evidence,
recovery material is independently available, no temporary weaker unlock path
remains, and the chosen policy supports our unattended update workflow.

References: [CoreOS encrypted root](https://github.com/coreos/fedora-coreos-docs/blob/main/modules/ROOT/pages/storage.adoc#encrypted-storage-luks),
[Ignition LUKS](https://coreos.github.io/ignition/operator-notes/#luks),
[Clevis TPM policy](https://github.com/latchset/clevis/blob/master/src/pins/tpm2/clevis-encrypt-tpm2.1.adoc),
[systemd enrollment policies](https://github.com/systemd/systemd/blob/main/man/systemd-cryptenroll.xml),
[PCR registry](https://uapi-group.org/specifications/specs/linux_tpm_pcr_registry/),
[libvirt TPM devices](https://libvirt.org/formatdomain.html#tpm-device).
