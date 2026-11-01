# Clamps firmware maintenance

## Observed 2026-11-01

Physical Clamps has AMI BIOS `MS2V003` (2026-05-22). `fwupdmgr get-upgrades`
offers UEFI Secure Boot `dbx` revocation-database updates; it does not currently
offer a system BIOS update. The metadata refresh timer is already enabled;
refreshing metadata does not install firmware. No firmware was staged or applied.
The operator is travelling without console access.

Root is bound to SHA-256 PCR 7. A Secure Boot database change can invalidate the
current TPM unlock policy. Keep firmware installation manual until an approved
recovery/re-enrollment procedure has been tested. `--no-reboot-check` suppresses
the reboot prompt; it does not guarantee staging-only behavior for every device.
See [fwupd FDE guidance](https://github.com/fwupd/fwupd/wiki/Full-Disk-Encryption-Detected),
[dbx/FDE issue](https://github.com/fwupd/fwupd/issues/7939) and
[fwupd behavior](https://github.com/fwupd/fwupd).

## TODO — with physical console access

1. Review the offered dbx release, current bootloader compatibility, verified
   independent recovery key and current-volume LUKS header backup before applying.
2. Apply the approved update in a supervised maintenance window. Recover and
   deliberately re-enroll the intended PCR 7 policy if measurements change;
   never remove PCR restrictions to make the update pass.
3. Verify Secure Boot, normal boot, unattended TPM unlock and remote access after
   a further cold boot. Refresh the recovery/header backup after enrollment.
4. Check official Beelink firmware for the exact board/revision against the
   installed BIOS. Do not assume a generic Mini S firmware is compatible or that
   a BIOS update is required merely because dbx updates exist. Review restore-on-AC
   and any unresolved physical power-off behavior at the same console visit.
5. Only then consider an opt-in, per-device firmware update policy. Separate
   metadata checks/downloads, online installs and reboot-staged capsule updates;
   keep boot-chain/TPM/dbx changes supervised until unattended recovery is proven.
   Leave automatic firmware installation and automatic reboot disabled for now.

Related: [TPM acceptance](TPM-ENCRYPTED-ROOT.md),
[Clamps cutover](CLAMPS-MIGRATION.md).


## Follow-up — research TPM-safe automation

The operator wants this explored before firmware updates become automatic.
Inventory update classes and their actual measured-boot effects first; a dbx or
BIOS update is not equivalent to an OS rebase. Evaluate whether approved future
PCR 7 states can be predicted/enrolled with the installed Clevis/initramfs stack,
or whether a different supported authorization policy is required. Do not assume
prediction or policy migration is available merely because another TPM tool
supports it. Retain recovery and the working enrollment until acceptance succeeds.

Use an owned disposable UEFI/swtpm VM to rehearse a controlled Secure Boot policy
change, interruption/retry, wrong-state denial, offline cold boot and independent
recovery. Then perform a supervised physical update: VM measurements cannot prove
vendor firmware behavior. Exit criteria are an approved update that unlocks
unattended under the intended strict policy, rejects unapproved states, preserves
data/recovery, and never falls back to empty PCRs or a plaintext on-disk unlock
key. Leave installation, firmware scheduling and policy migration unimplemented
until that evidence exists. Metadata checks may continue normally.
