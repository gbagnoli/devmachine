# TPM root remaining acceptance

Status reconciled 2026-11-02. Native LUKS2/Clevis PCR 7 and plain-root profiles,
QEMU UEFI/TPM lifecycle and independent vault recovery/header tooling are
implemented. Clamps has completed fresh installation on its replacement 1 TB
SSD; this is no longer a pending install trial.

The 2026-10-31 physical retry verified the initial Fedora XFS root, passed all
bootstrap transitions into signed uCore without manual recovery, reported
`target boot verified: true`, mounted encrypted Btrfs `/data`, and completed base
and subsequent credential/application delivery. This accepts the fresh physical
bootstrap path. On 2026-11-02 the operator additionally confirmed cold power-on
works, accepting unattended startup on the replacement SSD. Networkless boot,
rollback and recovery rehearsal remain separate. See [dated evidence](archive/ACCEPTANCE.md).

Earlier stock-SSD assisted recovery/cleanup and successful rebase evidence remain
historical, distinct from this clean replacement-disk installation. Previous failed
trials, measured MOK-authority difference, implementation details and superseded
retry instructions are [archived](archive/CLAMPS-BEFORE-SSD-RECONCILIATION.md).
There is no instruction to reinstall again or restore old SSD data.

Operating procedures: [TPM/recovery](../skillet/crates/workstation/docs/tpm.md),
[hardware installation](../butane/HARDWARE.md).

## Current upgrade incident

On 2026-11-23 the operator reported an unattended Fedora 45 upgrade failed TPM
unlock and left Clamps unavailable for four days. Fedora 44 unlocks automatically;
Fedora 45 can be unlocked with the retained temporary LUKS passphrase. Its journal
records two failures: slot 0 reports an invalid unsealed JWK, and slot 1 fails
its TPM policy check (`0x99d`). The operator measured identical SHA-256 PCR 7
values on Fedora 44 and manually unlocked Fedora 45. Both initramfs images
include the Rust decryptor. Older Clevis executables also fail with Fedora 45's
shared libraries; this is not a complete older-stack test.

[Upstream issue #46](https://github.com/latchset/clevis-pin-tpm2/issues/46)
identifies the matching Fedora 45 PCR-bound JWK regression: the TPM library's
policy-session nonce handling corrupts parameter-decrypted responses. The
packaged fix is `clevis-pin-tpm2-0.5.5-2.fc45`, with upstream confirmation that
PCR-bound decryption works without changing enrollment. Clamps has `0.5.5-1`.
Slot 1 may be a stale bootstrap binding; do not replace or delete either slot
based on these observations.

The ucore-images common recipe now requires the fixed package floor and
regenerates initramfs so early boot receives the new decryptor. The operator
successfully decrypted existing slot 0 on Fedora 45 using the staged patched
PIN (`patched-clevis exit=0`), with plaintext discarded and no enrollment changes.
This confirms the retained token works with the fixed software. Publication and
unattended boot from the rebuilt initramfs remain pending; userspace token
success does not establish early-boot acceptance.
The automatic update timer was stopped for this investigation; booting again
can reactivate an enabled timer. Preserve the working Fedora 44 deployment and
the independent recovery passphrase until the fixed path passes acceptance.
This incident leaves the OS update/rebase acceptance gate open.

[Automatic failed-boot recovery](FAILED-BOOT-RECOVERY.md) is a separate planned
follow-up: bounded early-boot failure, bootloader fallback and failed-image
quarantine. Existing unattended cold-boot success does not establish this behavior.

## Remaining matrix

| Check | Required evidence |
| --- | --- |
| Current-volume recovery records | Independently verify recovery key/header and volume identity for the replacement 1 TB SSD; old-disk recovery evidence does not transfer |
| Networkless cold boot | Root/data unlock and base storage work with NICs disconnected after images/bootstrap are present; network-dependent apps are separate |
| Approved OS update/rebase/rollback | Unattended unlock and unchanged data/credentials on the replacement installation across the agreed image matrix; older stock-disk update evidence is separate |
| Covered-policy change | Expected fail-closed TPM unlock followed by independently verified recovery and deliberate rebinding; never weaken to empty PCRs |
| Physical rescue/TPM replacement | Verified current volume, independent passphrase/header and separately enrolled replacement TPM while preserving recovery |
| New bootstrap prerequisite in UEFI VM | A fresh named VM must actually select the physical-media prerequisite; retained native TPM profiles and software-TPM container tests do not establish this |
| Flatpak UEFI/TPM equivalence | Parked separately from native libvirt results, with independently owned TPM/NVRAM artifacts |

Native VM fresh/repeat/cold-boot/recovery evidence and real-tool software-TPM
container tests remain valid for their recorded scopes. CI exercises the real
compiler and synthetic current/future-PCR, interrupted-import and policy-denial
checks without a vault. Physical success does not silently close unrun backend
checks. Plain root remains supported.

Native UEFI/swtpm kexec acceptance passed on 2026-11-02: signed-kernel rejection/
loading, same/different-kernel handoffs, forward update, rollback, automatic PCR 7
root unlock, encrypted systemd credential use and a later full firmware reboot.
This is not physical Clamps kexec acceptance or the physical-media bootstrap
prerequisite test. [Activation plan](IMAGE-OWNED-CONFIGURATION.md).

Firmware/dbx work requires [supervised maintenance](CLAMPS-FIRMWARE.md), current
recovery material and console access. Before destructive recovery, verify the
intended volume, preserve the source unlock path and disable secret-bearing
console recording. Never restore a header onto a running root.

Tang, custom UKIs and signed-PCR-policy whole-machine integrity remain separate
projects. Ordinary convergence never formats storage or replaces TPM bindings.
Remote bare-metal TPM support must be assessed individually.
[Storage decision](../design/storage.md).
