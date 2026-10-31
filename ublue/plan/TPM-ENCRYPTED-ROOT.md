# TPM root remaining acceptance

Native LUKS2/Clevis PCR 7 and plain-root profiles, QEMU UEFI/TPM lifecycle,
independent vault recovery/header backup and native TPM-loss recovery are
implemented. Native VM fresh/repeat/cold-boot/recovery and stock-SSD physical
signed boot/unattended unlock have evidence. The physical image transition
required recovery and rebinding under the final policy; it was not an
operator-free bootstrap success. Preserve [the original record](archive/TPM-ENCRYPTED-ROOT.md)
and [acceptance evidence](archive/ACCEPTANCE.md).

Operating procedures: [TPM/recovery](../skillet/crates/workstation/docs/tpm.md),
[hardware installation](../butane/HARDWARE.md).

## Physical cleanup accepted

On 2026-10-14 the operator confirmed removal of the temporary short passphrase,
retained recovery/TPM verification, refreshed and backed-up vault header, and
unattended reboot on the stock disk. The operator also confirmed a successful
signed image rebase/reboot onto the moving `:latest` tag. Rollback and wider
data/credential preservation checks remain open. See [acceptance](archive/ACCEPTANCE.md).

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

## Deferred scope

Tang, custom UKIs and signed-PCR-policy whole-machine boot integrity are separate
projects. The accepted native scope is encrypted-disk loss/removal; ordinary
convergence never formats storage or replaces TPM bindings. Remote bare-metal
hosts must be assessed individually; plain root remains supported.
[Storage decision](../design/storage.md).
