# Syncthing service

Run Syncthing as its own Quadlet container on the host's declared bridge, with
`ContainerName=syncthing`. Caddy can reach its UI at
`http://syncthing:8384`. The GUI port is not published on the host;
peer-transfer ports remain published. Tailnet-restricted Caddy provides UI access. See the [private UI design](private-ui-access.md).

Mount `/var/lib/data/syncthing` at `/var/syncthing`. Keep the upstream
directory layout there, including `config/` and synced folders, so a
state transfer can retain the device identity and folder definitions. The
directory is a Btrfs subvolume owned by the declared host account. Syncthing
runs as container UID/GID 1000; Podman's user namespace maps that container
identity to the host account and its primary group. Keep host ownership,
container identity, and namespace mapping explicit and separate. Never copy
a source server's live identity into a disposable VM.

The bridged container can limit LAN discovery compared with host networking.
This follows the existing Chef port-published container and keeps service-name
DNS available; real peer connectivity is separate from bridge configuration.

References: [official Syncthing container guidance](https://github.com/syncthing/syncthing/blob/main/README-Docker.md),
[shared storage design](storage.md), [Pi-hole network design](pihole-network.md).
