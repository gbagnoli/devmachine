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

## Remaining matrix

| Check | Required evidence |
| --- | --- |
| Physical keyslot cleanup | Remove the temporary short passphrase deliberately; retain verified independent recovery and final TPM binding; refresh/read back the vault header after changes |
| Fresh bootstrap under final policy | All approved transition boots unlock unattended, or explicitly document the recovery/rebind boundary; do not infer PCR behavior from ostree signature labels |
| Networkless cold boot | Root/data unlock and base storage work with NIC disconnected after images/bootstrap are present; network-dependent apps are a separate check |
| Approved OS update/rebase/rollback | Unattended unlock with unchanged data/credentials across the agreed deployed-image matrix; bounded failure and recovery when policy changes |
| Covered-policy change | Expected fail-closed TPM unlock followed by independently verified recovery and deliberate rebinding; never weaken to empty PCRs as a shortcut |
| Physical rescue/TPM replacement | Verified disk/volume identity, independent passphrase/header, separately enrolled replacement TPM and intact recovery path |
| Final disk deployment | Repeat media/install/recovery acceptance on the selected production disk; stock-SSD tests do not accept an uninstalled replacement |
| Flatpak UEFI/TPM equivalence | Keep separate from accepted native libvirt results; use independent owned TPM/NVRAM artifacts |

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
