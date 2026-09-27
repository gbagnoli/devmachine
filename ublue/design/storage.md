# Shared uCore persistent storage

Decision: use the Btrfs filesystem backing uCore's writable `/var` and
`/sysroot`, with a `data` subvolume mounted at `/var/lib/data`. This follows
boxy's shared-space OS/data layout while preserving uCore's deployment layout.
This layout is shared by all uCore hosts. On the disposable clamps VM, `/` is a
composefs overlay and `/var` is on the Btrfs root partition; an `/os` root
subvolume must not be copied from boxy. The physical disk for clamps is still
undecided.

`/var/lib/data` is the host's canonical service-state root. Create child
subvolumes only for independently managed data: initially `pihole` and
`containers`. Pi-hole's container `/etc/pihole` maps to
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
must list service subvolumes explicitly. The `containers` subvolume can then
be omitted from service-data snapshots. A shared filesystem shares free space
and physical failure; snapshots are not independent backups.

The clamps VM passed fresh provisioning, mount, SELinux, reboot, repeat apply,
and unavailable/wrong-mount recovery. A post-provision OS rebase and the
physical disk choice remain open in
[the storage plan](../plan/CLAMPS-STORAGE-CREDENTIALS.md).

References: [XDG base directories](https://specifications.freedesktop.org/basedir/),
[FHS `/srv`](https://specifications.freedesktop.org/fhs/latest/srv.html),
[Btrfs subvolumes](https://btrfs.readthedocs.io/en/stable/btrfs-subvolume.html).
