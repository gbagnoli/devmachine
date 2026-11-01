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
