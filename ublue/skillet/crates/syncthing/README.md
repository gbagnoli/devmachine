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

## Initial device configuration

Remote `skillet apply <host>` resolves the device name as `<host>.<zone>` using
`skillet/environments/<environment>/dns/cloudflare-zone-id` and the existing
Cloudflare token creator's Zone Read permission. The UI prefix is not used.
No additional Syncthing password or API key needs to be added to the vault.

The encrypted `syncthing_initial` credential feeds
`skillet-syncthing-initialize.service` before full application starts the
container. It generates complete upstream configuration offline, preserves
existing device keys, and atomically installs XML only when it is absent.
Existing configuration is never updated by this bootstrap, including later
folder/device changes or GUI passwords. Interrupted publication retains private
staging under `config/.skillet-initial` for retry.

Identity-protected UIs initialize `gui.insecureAdminAccess=true` to acknowledge
proxy authentication. This suppresses the GUI banner; Syncthing may still log a
startup warning. CSRF protection and the private backend remain enabled.
Disposable provisioning with UI/DDNS uses the test zone and recorded VM hostname;
provider-free VM provisioning uses an explicit `example.invalid` fixture name.

For a reset, stop Syncthing and back up `config/config.xml` before removing only
that file. Preserve `cert.pem`, `key.pem`, the database and synchronized files.
Then activate the initializer and restart Syncthing. See the
[design](../../../design/syncthing.md) and
[acceptance plan](../../../plan/archive/SYNCTHING-INITIAL-CONFIG.md).
