# Syncthing

Syncthing is shared fleet policy. The caller supplies storage/account/namespace
inputs. State is a Btrfs subvolume at `/var/lib/data/syncthing`; configuration
and user data remain separate beneath it. The default user-file root is `data`,
and new instances default to `data/Sync`. Existing folders must be migrated or
configured explicitly.

Use [remote apply](../workstation/README.md#apply-a-production-host) or
[owned VM provisioning](../vm/README.md). No application vault entry is required.
The UI is `https://syncthing.<host>.<ui-domain>` through [Caddy](../caddy/README.md).
Both canonical and `sync.<host>.<ui-domain>` aliases require the complete
Tailscale user allowlist at `skillet/tailscale/ui-users`. Tagged server devices,
unknown users and lookup failures are denied. Caddy and Syncthing share a
dedicated strict backend network; unrelated service containers do not join it.
Use normal remote apply to deploy the changed host definition and redeliver
Caddy's matching encrypted policy. GUI credentials, keys and folders are preserved;
existing native GUI passwords are not cleared automatically.

GUI port 8384 is not published; peer-transfer port 22000 is published over TCP/UDP
on IPv4/IPv6. Inspect `syncthing.service` and its journal.

Host ownership follows the declared account. The container identity (normally
UID/GID 1000) is mapped explicitly through Podman's user namespace; do not
recursively chown existing synchronized files to the container's unmapped IDs.
Stop the source instance before migrating its production identity. Never restore
that live identity into an ordinary smoke VM.

[Mikochi](../mikochi/README.md) browses only the declared user-file root.
[btrbk](../btrbk/README.md) snapshots caller-selected leaf subvolumes; nested
subvolumes are unsupported.

Design: [Syncthing](../../../design/syncthing.md).
