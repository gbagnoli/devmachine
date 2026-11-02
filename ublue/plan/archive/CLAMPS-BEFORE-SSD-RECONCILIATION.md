# Superseded Clamps plan snapshot — 2026-11-02

These are the previous active instructions, retained as history. The replacement
1 TB SSD is installed; use the [active plans](../README.md), not these earlier work orders.
They include failed trials superseded by the successful 2026-10-31 installation.

## Previous CLAMPS-MIGRATION.md

# Clamps migration and cutover

Current implementation and source parity are in [the parity checklist](../CLAMPS-PARITY.md).
Stock-SSD installation reached signed uCore, an encrypted Btrfs data mount,
TPM unattended boot and base configuration. The operator confirmed physical
wired/Wi-Fi configuration and Wi-Fi connectivity after deployment/apply.
Operator container observations do
not establish production service-data migration or client reachability.
See [historical evidence](../archive/ACCEPTANCE.md) and
[the previous milestone record](../archive/CLAMPS-MIGRATION.md).

## Work order

1. Wired/Wi-Fi setup and Wi-Fi connectivity are operator-confirmed. Finish the
   remaining [networking checks](../HOST-NETWORKING.md): client DHCP resolver use,
   resolver outage behavior, permanent MAC and management recovery. DNS answers
   from Rupik, route preference, reboot and Wi-Fi switching are now confirmed;
   see the [acceptance log](../archive/ACCEPTANCE.md). Pi-hole AAAA serving is
   verified in an isolated v6 container. LAN IPv6 client reachability is not
   enabled or applicable until the LAN has usable IPv6.
2. Temporary passphrase cleanup, recovery/header verification and unattended
   reboot are operator-confirmed. Remaining [TPM checks](../TPM-ENCRYPTED-ROOT.md)
   include networkless boot, rollback and wider update preservation checks; one
   signed image rebase/reboot is operator-confirmed. Stock-SSD acceptance is separate
   from final production disk selection/installation.
3. Private Pi-hole v6 delivery is implemented through the shared environment DNS
   inventory and dedicated encrypted phase. Dual-stack serialization and Pi-hole
   AAAA answering are verified. Complete the vault migration to `dns/local-prefix`
   and `dns/entries`, verify intended production entries, then migrate Pi-hole
   state. Runtime validation is recorded in the acceptance log.
4. Inventory and migrate production data/identities from rupik. Stop source
   Syncthing/UniFi identities before enabling replacements. Retain the source
   SSD and independent backups until rollback/cutover is accepted. Follow
   [UniFi](../CLAMPS-UNIFI.md), [DDNS](../CLAMPS-CLOUDFLARE-DDNS.md) and
   [private UI](../GENERIC-PRIVATE-UIS.md) handover gates.
5. Validate complete production remote apply with matching guest artifacts:
   [operating guide](../../skillet/crates/workstation/README.md#apply-a-production-host).
   The operator reports the new deployment/apply workflow worked. Repeat apply
   and individual production service/data acceptance remain separate checks.
6. Verify physical repeat apply, reboot and an approved OS update; client access,
   peer synchronization, UniFi devices/cloud access, and SSH/sudo/ET. Follow
   [baseline parity](../CLAMPS-PARITY.md). Approve the production exit node; routed
   traffic exercises remain omitted by operator decision.
7. Complete automated provisioning/recovery before
   [access finalization](../../skillet/crates/cli-common/README.md#administrative-access).
   Verify first-login password change and password-required sudo interactively.
   If older media installed `force-pw-change.service`, stop/disable and remove
   that obsolete expiry unit before adopting explicit finalization.
8. Agree an outage/rollback window and final disk; establish off-machine data
   backup retention and rehearse restore before retiring rupik. Local btrbk
   snapshots are not an independent backup.

## Deferred live acceptance batch

Run these with suitable clients and the required private configuration:

- Pi-hole IPv4 LAN ingress and UDP/TCP answer behavior passed from Rupik; an
  isolated Pi-hole v6 instance served a synthetic AAAA over UDP/TCP. Client
  IPv6 ingress remains deferred until usable LAN IPv6 exists. Still test normal
  DHCP-provided resolver use and local-resolver/bootstrap outage behavior.
- Private UI DNS/HTTPS from actual tailnet clients and Caddy denial from a
  separate network outside the tailnet; guest loopback denial is insufficient.
- Cloudflare production issuance and bounded renewal, preserving owned records.
- Physical UPS shutdown/restart and NUT timer/SELinux/reboot/cloud metric recovery:
  [monitoring acceptance](../CLAMPS-DATADOG.md). No forced shutdown in routine smoke.
- Interrupted SSH credential streaming/encryption: preserve or recover the
  usable full credential, retry through the normal owner, and verify consumer
  usability. Atomic-save/activation-failure tests do not cover this interruption.

The SSH-only user-mode VM cannot prove real LAN ingress. Preserve each parked
check explicitly; [refactor acceptance](../SKILLET-REFACTOR.md) remains separate.
Broader OS hardening is a placeholder, not an accepted Chef port. Confirm any
retained legacy NAT routes/WOL targets privately rather than guessing them.

## Separate optional work

[Home Assistant](../HOME-ASSISTANT.md) has an implemented optional module; its
physical onboarding and remaining acceptance are separate. It is not a
rupik-retirement gate. Shared SMTP is implemented; [its follow-ups](../CLAMPS-EMAIL.md)
and parked UniFi recovery email are separate. [Joey](../JOEY-MIGRATION.md) may
proceed independently, preserving its own source and acceptance boundaries.


## Previous TPM-ENCRYPTED-ROOT.md

# TPM root remaining acceptance

Native LUKS2/Clevis PCR 7 and plain-root profiles, QEMU UEFI/TPM lifecycle,
independent vault recovery/header backup and native TPM-loss recovery are
implemented. Native VM fresh/repeat/cold-boot/recovery and stock-SSD physical
signed boot/unattended unlock have evidence. The physical image transition
required recovery and rebinding under the final policy; it was not an
operator-free bootstrap success. Preserve [the original record](../archive/TPM-ENCRYPTED-ROOT.md)
and [acceptance evidence](../archive/ACCEPTANCE.md).

Operating procedures: [TPM/recovery](../../skillet/crates/workstation/docs/tpm.md),
[hardware installation](../../butane/HARDWARE.md).

## Physical cleanup accepted

On 2026-10-14 the operator confirmed removal of the temporary short passphrase,
retained recovery/TPM verification, refreshed and backed-up vault header, and
unattended reboot on the stock disk. The operator also confirmed a successful
signed image rebase/reboot onto the moving `:latest` tag. Rollback and wider
data/credential preservation checks remain open. See [acceptance](../archive/ACCEPTANCE.md).

Firmware/dbx changes are deferred to [supervised maintenance](../CLAMPS-FIRMWARE.md)
while the operator lacks physical console access.

## Remaining matrix

| Check | Required evidence |
| --- | --- |
| Fresh bootstrap under final policy | All approved transition boots unlock unattended on the replacement disk; historical assisted recovery does not accept a clean installation |
| Networkless cold boot | Root/data unlock and base storage work with NIC disconnected after images/bootstrap are present; network-dependent apps are a separate check |
| Approved OS update/rebase/rollback | Unattended unlock with unchanged data/credentials across the agreed deployed-image matrix; bounded failure and recovery when policy changes |
| Covered-policy change | Expected fail-closed TPM unlock followed by independently verified recovery and deliberate rebinding; never weaken to empty PCRs as a shortcut |
| Physical rescue/TPM replacement | Verified disk/volume identity, independent passphrase/header, separately enrolled replacement TPM and intact recovery path |
| Final disk deployment | Repeat media/install/recovery acceptance on the selected production disk; stock-SSD tests do not accept an uninstalled replacement |
| Flatpak UEFI/TPM equivalence | Keep separate from accepted native libvirt results; use independent owned TPM/NVRAM artifacts |

## Priority: clean installation on the replacement SSD

The operator reports another TPM unseal failure during installation on the new
1 TB SSD and explicitly prefers fresh reinstall trials over repairing that
installation. No old service data is to be restored. The previous disk's vault
recovery key/header does not apply to this new volume. Photos collected on
2026-10-31 show different SHA256 PCR 7 values for the failing and selected
previous boot entries, both reporting kernel `7.2.9-201.fc44.x86_64`.
`tpm2_pcrread` is available in the initramfs; `tpm2_eventlog` is not. Both probes
used `rd.break=initqueue rd.luks=0`. Raw logs subsequently collected with
`skillet/scripts/tpm-boot-report.sh` were replayed locally. Both SHA256 PCR 7
replays exactly match the corresponding TPM readings. All earlier PCR 7 events
match; the failing boot adds one `EV_EFI_VARIABLE_AUTHORITY` event for the
Universal Blue kernel certificate in `MokListRT`. Extending the previous PCR
with that event reproduces the failing PCR exactly. Secure Boot/PK/KEK/db/dbx
and earlier signing-authority events are unchanged. This identifies the actual
measured transition, rather than an ostree signature-label difference.
Original enrollment metadata was not exported from that attempt. The new
implementation validates the original sealed object's policy during fresh boot
before any enrollment mutation. Fresh physical installation remains unverified.

1. Before erasing this attempt, identify whether failure occurred on the first
   installed FCOS boot or after a bootstrap rebase. From a root emergency shell
   on the failed boot, collect `uname -r`, `/proc/cmdline`, availability of
   `tpm2_pcrread`/`tpm2_eventlog`, and `tpm2_pcrread sha256:7` when available.
   Preserve the firmware event log for comparison if tools permit. Do not
   change firmware, clear TPM state, rebind slots or print decrypted keys.
2. Compare the failed boot against the initial enrollment boot. Shim measures
   matching binary-signing authorities into PCR 7, so pre-enrolling a MOK alone
   does not establish that FCOS and uCore have identical PCR 7 values. Identify
   actual authority/policy events rather than inferring the cause from an
   ostree signature label. If evidence is unavailable, instrument the next
   empty-disk installation to retain nonsecret measurements before each rebase.
   The selected previous/failing log comparison is now verified as described
   above; the reports are private workstation artifacts and are not committed.
3. Select and implement the enrollment/install ordering from that evidence.
   **Implemented:** physical TPM media orders a separate Rust enrollment unit
   before `ucore-bootstrap.service`. It verifies log replay and the current
   native TPM object's authorization policy, predicts the supplied trusted MOK
   authority event, and journals a sealed target envelope before adding its
   keyslot/token. It preserves the original path and verifies the target on
   the next approved boot. Completed journals do not recreate later-removed
   slots. There is no machine-specific PCR constant or manual PCR flag.
   The installed Fedora pin silently ignored supplied future PCRs in a real-tool
   check, so target envelopes are constructed with the existing TPM tools and
   JOSE format instead. Their actual public-object policy and attributes are
   checked, and Fedora Clevis decrypts them in acceptance. Plaintext exists only
   in memory or private `/run` tmpfs. Plain-root/retained VM profiles are unchanged.
   Signed-image verification and SHA256 PCR 7 restrictions remain required.
   No unrestricted slot, plaintext disk key or manual recovery bridges the boot.
4. Validate a fresh named UEFI/TPM VM through all bootstrap boots, then repeat
   on the empty physical SSD. Acceptance requires unattended first install,
   final signed `:latest` deployment, root/data mounts, repeat cold boot, and
   verified independent recovery for the new volume. Plain-root installs must
   remain supported. VM results cannot substitute for S13 firmware evidence.

The software-TPM container acceptance does not require KVM: it exercises actual
Fedora Clevis/cryptsetup, restricted current/future unlock, interrupted import,
repeat behavior, original-token preservation, recovery through a non-first
working binding and denial after an additional unapproved PCR extension.
The actual private boot reports pass Rust prediction/replay, and strict Butane
compilation preserves the base bootstrap unit plus its new prerequisite.
CI runs the synthetic real-tool and compiler checks without a vault.
No fresh UEFI VM or physical acceptance is claimed for the new flow; check
runtime capability separately and do not install workstation packages.
An escalated read-only recheck confirmed `/dev/kvm` and the existing native
`qemu:///session` runtime are available; the sandbox-only absence was not a
workstation limitation. The existing Donbot VM was left untouched. A normal
retained TPM VM profile does not exercise the new physical-media prerequisite,
so its earlier acceptance must not be reported as acceptance of this change.

References: [shim TPM measurements](https://github.com/rhboot/shim/blob/main/README.tpm),
[Clevis explicit PCR values](https://github.com/latchset/clevis/blob/master/src/pins/tpm2/clevis-encrypt-tpm2.1.adoc).

Before destructive recovery, persist an independent key and protected off-machine
header/archive, verify their bytes against the intended volume, preserve the
source unlock path, and disable secret-bearing console recording. Never restore
a header onto a running root. Record exact actual results and unresolved checks.

## Replacement-disk first-boot retry

The next operator report failed in initramfs partition-save, before the TPM
bootstrap unit. The saved report shows `ignition.firstboot` and an already
encrypted root. The copied boot configuration contains only one initial Fedora
entry, no saved GRUB override, a retained first-boot marker and no encrypted-root
kernel arguments. This excludes selection of an older listed deployment; it
does not establish what interrupted the preceding storage conversion. The
operator is unsure whether an intermediate reboot was missed. Do not label this
as the earlier PCR mismatch or claim that stale competing signatures were found.

Implemented guards for the next fresh trial: live installation verifies a
single labelled initial root filesystem before reporting success, and uCore
bootstrap requires/orders after CoreOS first-boot completion. Capture the
installer's verification line and preserve the first failure report before
any manual reboot. A new bundle/USB is required for the dependency change.
Fresh physical acceptance remains pending; no repair/wipe of the failed disk
was performed by the agent.

## Deferred scope

Tang, custom UKIs and signed-PCR-policy whole-machine boot integrity are separate
projects. The accepted native scope is encrypted-disk loss/removal; ordinary
convergence never formats storage or replaces TPM bindings. Remote bare-metal
hosts must be assessed individually; plain root remains supported.
[Storage decision](../../design/storage.md).


## Previous HOST-NETWORKING.md

# Native host networking

## Implemented

Caller-selected wired/Wi-Fi profiles; structured vault fields; format, host and
subnet validation; encrypted production delivery; separate guest preparation;
boot ordering; volatile root-only keyfiles; DNS-order preservation. Clamps opts
in to both interfaces. Other hosts and smoke VMs retain DHCP. Remote all-service
apply prepares networking without switching the active connection. See
[design](../../design/host-networking.md).

## Physical acceptance status

On 2026-10-13 the operator confirmed successful provisioning and correctly
configured wired/Wi-Fi on physical clamps, including a successful Wi-Fi test.
On 2026-10-20, Rupik verified Pi-hole DNS and generated host answers through
both clamps addresses, and host inspection confirmed wired route preference.
The operator also confirmed a reboot and Wi-Fi switch worked the previous day.
See [acceptance evidence](../archive/ACCEPTANCE.md). Permanent-MAC matching,
normal DHCP-provided resolver use, Pi-hole outage behavior and management
recovery remain open.

## Detailed acceptance checklist

1. Install matching generic and host binaries using the existing deployment
   paths. Verify the deployed image has NetworkManager Wi-Fi support and a
   supplicant. Add missing packages to the image repository, not the workstation.
2. Deliver networking with explicit preparation using the README command.
   Confirm root-only runtime profiles and encrypted persistent input. Fresh live
   profiles must have autoconnect disabled; current addresses and management
   connectivity must remain unchanged. Do not log profile contents or PSKs.
3. First exercise ordering and repeat preparation in an isolated disposable VM,
   with fixture inputs and disposable NICs, preserving the management NIC. Never
   deliver production network entries to test VMs.
4. With local-console recovery available, reboot physical hardware. Verify
   preparation precedes NetworkManager, static Ethernet address, requested DNS
   order, local Pi-hole over loopback and independent DNS before Pi-hole startup.
5. Verify permanent-MAC matching and management recovery with Ethernet attached
   and detached. Wi-Fi association/static address and route preference with
   both links active are confirmed; normal client DNS use and Pi-hole outage
   behavior remain to be checked. Repeat preparation and another unattended
   boot after the remaining profile checks.

Offline parser validation and injected tests do not establish physical wireless
association or actual reboot reachability. Record evidence separately in
`plan/archive/ACCEPTANCE.md`. Live switching/checkpoint rollback remains future work.

Physical binary updates now use `skillet deploy HOST --target USER@HOST`;
remote `apply` performs that deployment by default. Network-only delivery still
requires running the guest base phase once to install the preparation unit.
