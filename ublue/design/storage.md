# Shared uCore persistent storage

Decision: use the Btrfs filesystem backing uCore's writable `/var` and
`/sysroot`, with a `data` subvolume mounted at `/var/lib/data`. This follows
boxy's shared-space OS/data layout while preserving uCore's deployment layout.
This layout is shared by all uCore hosts. On the disposable clamps VM, `/` is a
composefs overlay and `/var` is on the Btrfs root partition; an `/os` root
subvolume must not be copied from boxy. The physical disk for clamps is still
undecided.

`/var/lib/data` is the host's canonical service-state root. Create child
subvolumes only for independently managed data: initially `pihole`,
`syncthing`, and `containers`. Pi-hole's container `/etc/pihole` maps to
`/var/lib/data/pihole/etc`; any retained dnsmasq configuration and logs live
under that Pi-hole tree. Rootful Podman's graphroot is
`/var/lib/data/containers/system` on fresh installs, with runroot under
`/run/containers/storage`. Chef set `driver=btrfs` for rupik and boxy; uCore
currently uses overlay, so the shared config keeps overlay while moving the
graphroot. Its SELinux file-context equivalence matches the former graphroot.
Rootless storage is added only for a host that needs rootless containers.

XDG base directories describe user-specific files, so they do not improve the
location of system service state. `/srv` suits site data served directly to
clients; Pi-hole's database and Podman's image layers are internal state.
`/var/lib/data` also matches boxy's and calculon's existing convention.

The shared `butane/includes/data-storage.bu` include creates the top-level
`data` subvolume on the Btrfs filesystem backing `/var`, mounts it, prepares
the `containers` subvolume, and configures rootful Podman. It checks that the
standard `root` partition
label points to that same filesystem. Each host's Butane template must merge
the include; clamps currently does. Host Skillet creates application
subvolumes and directories. Existing populated Podman stores require an
explicit migration; changing an installed host's graphroot is not automatic.
Skillet must not format a populated disk.
Skillet, managed application services, and Podman auto-update require the
mounted data subvolume and fail when it is absent, preventing their writes to
a fallback directory. A manually invoked Podman command does not have this
unit dependency and must only run after validating the mount.
Btrfs snapshots do not include nested subvolume contents, so backup policy
must list service subvolumes explicitly. Snapshot source subvolumes are leaf
subvolumes: nested subvolumes inside them are unsupported. Syncthing follows
this rule by design. The `containers` subvolume can then be omitted from
service-data snapshots. A shared filesystem shares free space and physical
failure; snapshots are not independent backups.

## Planned encryption

Decision, 2026-10-02: add LUKS2 beneath the shared Btrfs backing filesystem
and prefer local TPM2 unlock on both local and remote physical hosts. Normal
boots must not need a network or operator. This protects persistent service
state, including Podman's disk copies of secrets, while retaining the existing
subvolume layout. Tang is deferred; assess each remote host's TPM first.
EFI and `/boot` remain outside encryption. An explicit boot-integrity policy
and update/rollback compatibility must be proved before claiming protection
against theft of the whole machine. Keep an independent recovery key in the
private vault and a protected off-machine LUKS header backup.
Encryption is not implemented. Installation owns formatting; the current raw
root partition mount assumptions must be changed to the unlocked filesystem.
QEMU functional tests and physical acceptance are specified in the
[TPM encrypted-root plan](../plan/TPM-ENCRYPTED-ROOT.md).

The clamps VM passed fresh provisioning, mount, SELinux, reboot, repeat apply,
and unavailable/wrong-mount recovery. A post-provision OS rebase and the
physical disk choice remain open in
[the storage plan](../plan/CLAMPS-STORAGE-CREDENTIALS.md).

References: [XDG base directories](https://specifications.freedesktop.org/basedir/),
[FHS `/srv`](https://specifications.freedesktop.org/fhs/latest/srv.html),
[Btrfs subvolumes](https://btrfs.readthedocs.io/en/stable/btrfs-subvolume.html).


## TPM capability audit (2026-10-10)

The retained signed-uCore VM has Clevis 21, cryptsetup 2.8.8, systemd 259.8
with TPM2 support, and TPM2 tools. Its deployed initramfs contains Clevis TPM2,
cryptsetup/systemd TPM token support and Btrfs. No extra package installation is
currently justified for native Clevis unlock; actual encrypted-boot acceptance
must also check the initial FCOS image and each subsequent deployment.

The native Rocky test backend has KVM, swtpm and q35 OVMF Secure Boot firmware
with enrolled-key NVRAM templates. The workstation exposes no physical TPM;
virtual-TPM tests do not require one. Existing smoke domains boot BIOS without
TPM. New encrypted tests must explicitly select UEFI/Secure Boot, independent
TPM2 state and owned persistent NVRAM. Firmware capability is not boot acceptance.

Current boot entries use separate kernel/initramfs files under GRUB. Neither
ukify nor systemd-measure was found on the deployed guest; no signed PCR-policy
pipeline exists in our image recipes. PCR 7 binds Secure Boot policy, not all
kernel/initramfs/command-line bytes. TPM-only unlock must not be described as
protection against whole-machine theft or tampering. Signed-policy boot integrity
requires a separate implemented update/rollback chain; adding packages alone
would not establish it. The first profile's protection scope is awaiting the
operator's choice; no PCR enrollment or encrypted formatting has been performed.

Sources: [FCOS encrypted root](https://github.com/coreos/fedora-coreos-docs/blob/main/modules/ROOT/pages/storage.adoc#encrypted-storage-luks),
[Clevis TPM threat model](https://github.com/latchset/clevis/blob/master/src/pins/tpm2/clevis-encrypt-tpm2.1.adoc#threat-model),
[systemd PCR policies](https://github.com/systemd/systemd/blob/main/man/systemd-cryptenroll.xml).
