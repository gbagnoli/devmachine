# Clamps persistent storage

Decision: use the Btrfs filesystem backing uCore's writable `/var` and
`/sysroot`, with a `data` subvolume mounted at `/var/lib/data`. This follows
boxy's shared-space OS/data layout while preserving uCore's deployment layout.
On the disposable clamps VM, `/` is a composefs overlay and `/var` is on the
Btrfs root partition; an `/os` root subvolume must not be copied from boxy.
The physical disk for clamps is still undecided.

`/var/lib/data` is the host's canonical service-state root. Create child
subvolumes only for independently managed data: initially `pihole` and
`containers`. Pi-hole's container `/etc/pihole` maps to
`/var/lib/data/pihole/etc`; any retained dnsmasq configuration and logs live
under that Pi-hole tree. Rootful Podman's graphroot will be
`/var/lib/data/containers/system`, configured before its first application
container on a fresh install. Keep the working overlay storage driver unless
VM evidence supports a change. Rootless storage is added only for a host that
needs rootless containers.

XDG base directories describe user-specific files, so they do not improve the
location of system service state. `/srv` suits site data served directly to
clients; Pi-hole's database and Podman's image layers are internal state.
`/var/lib/data` also matches boxy's and calculon's existing convention.

The data mount and its identity are installation concerns. Skillet may create
and converge directories and subvolumes but must not format a populated disk.
Application startup and Podman storage use must require the mounted data
subvolume and fail when it is absent, preventing writes to a fallback directory.
Btrfs snapshots do not include nested subvolume contents, so backup policy
must list service subvolumes explicitly. The `containers` subvolume can then
be omitted from service-data snapshots. A shared filesystem shares free space
and physical failure; snapshots are not independent backups.

Implementation and mount-failure acceptance are planned in
[the storage plan](../plan/CLAMPS-STORAGE-CREDENTIALS.md).

References: [XDG base directories](https://specifications.freedesktop.org/basedir/),
[FHS `/srv`](https://specifications.freedesktop.org/fhs/latest/srv.html),
[Btrfs subvolumes](https://btrfs.readthedocs.io/en/stable/btrfs-subvolume.html).
