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
