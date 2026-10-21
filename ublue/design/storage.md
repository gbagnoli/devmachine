# Shared persistent storage and root protection

## Layout

Use uCore's Btrfs backing filesystem for writable /var and /sysroot, with a data
subvolume mounted at `/var/lib/data`. Keep uCore's composefs deployment layout;
boxy's `/os` subvolume is not copied onto this OS. Shared space follows the
existing fleet convention without a separate data partition.

Service state belongs under /var/lib/data, with independently managed leaf
subvolumes. Rootful Podman graphroot is `containers/system`, preserving uCore's
overlay driver and SELinux equivalence; runroot stays volatile. Rootless storage
is selected separately. XDG describes user files, not system container layers or
service databases; /srv is not needed for these internal state trees.

Ignition owns new-install formatting. The shared data include checks the backing
filesystem identity and prepares/mounts data and containers. Skillet converges
application directories/subvolumes without formatting existing disks. Managed
services and auto-update require the verified mount, preventing writes into a
fallback directory. Manual Podman is outside those unit dependencies.

Snapshots list selected leaf subvolumes explicitly; nested sources are unsupported.
Container layers are excluded. Shared space also shares device failure;
[btrbk](btrbk.md) history is not an independent backup. Existing populated stores
need explicit migration, not a graphroot edit followed by automatic data movement.

## Native root encryption

LUKS2 sits below Btrfs, preserving data layout and protecting persistent runtime
secret copies. Native Ignition/Clevis TPM2 SHA256 PCR 7 with Secure Boot enables
network/operator-free normal unlock. Plain root remains the default for hardware
without suitable TPM; explicit TPM selection fails on missing prerequisites.
Root profile is independent of host identity and never silently downgrades.

EFI and /boot remain outside encryption. The accepted scope is encrypted-disk
loss/removal, not authenticated whole-machine boot. PCR 7 measures Secure Boot
policy/authorities, not every kernel/initramfs/command-line byte. Ostree's image
verification label does not itself explain or fix TPM policy mismatches. Final
MOK/Secure Boot policy and independently verified recovery are required before
changing enrollment. Routine convergence never replaces TPM bindings.

Recovery uses a printable independent passphrase and protected volume-bound
header in the portable vault. Refresh headers after enrollment changes; a header
is not a data backup. Write-ahead persistence/readback protects the original
unlock path, including stopped clones with independent TPM/NVRAM and unlogged
recovery consoles. [Secret archives](secrets.md#root-recovery-archives).

## Installation boundary

Physical media selects an explicit whole-disk by-id path. Workstation generation
never opens block devices; the live helper verifies the actual server and asks
for destructive confirmation. Private bootstrap access replaces inherited keys
and hashes. The small live Ignition embeds access/helper/destination digest;
the full payload travels separately and is verified in root-only live tmpfs,
respecting CoreOS's embedding limit. VM /dev/vda remains a separate template.

Disk choice and hardware/update acceptance are in [clamps migration](../plan/CLAMPS-MIGRATION.md)
and [TPM acceptance](../plan/TPM-ENCRYPTED-ROOT.md). Stock-SSD installation evidence
must not accept an uninstalled final production disk. [Operating procedures](../butane/HARDWARE.md).

References: [XDG](https://specifications.freedesktop.org/basedir/),
[Btrfs](https://btrfs.readthedocs.io/en/stable/btrfs-subvolume.html),
[FCOS encryption](https://github.com/coreos/fedora-coreos-docs/blob/main/modules/ROOT/pages/storage.adoc#encrypted-storage-luks),
[Clevis threat model](https://github.com/latchset/clevis/blob/master/src/pins/tpm2/clevis-encrypt-tpm2.1.adoc#threat-model),
[PCR policies](https://github.com/systemd/systemd/blob/main/man/systemd-cryptenroll.xml).
