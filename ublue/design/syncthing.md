# Syncthing service

Run Syncthing as its own Quadlet container on the caller-declared dedicated
strictly isolated GUI backend bridge, with
`ContainerName=syncthing`. Caddy can reach its UI at
`http://syncthing:8384`. The GUI port is not published on the host;
peer-transfer ports remain published. Caddy and Syncthing are the only declared
members of that backend. The canonical hostname and every alias require the
shared LocalAPI WhoIs user allowlist before proxying, including cross-origin
checks and stripping caller-supplied identity headers. Tagged, unknown or
unlisted clients and daemon lookup failures are denied. See the
[identity design](tailnet-ui-authentication.md).

Application configuration, passwords, API keys, device identities and folders
are preserved. A deployment with no native GUI login gets tailnet-only login;
retained native GUI credentials are not silently cleared by network convergence.
Moving an existing container off the shared bridge requires its recreation;
Caddy redelivery joins the matching private bridge and installs the revised gate.
Old payloads that omit the declared protection are rejected, so normal remote
apply must regenerate the Caddy credential; do not manually retain the old one.

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

## Initial configuration

Bootstrap settings apply only while `config/config.xml` is absent. Syncthing owns
that file after initialization: subsequent provisioning preserves preferences,
shares and device definitions, even if bootstrap settings change. Initialize via
the upstream generator and publish the completed configuration atomically;
never overwrite an existing file or depend on an incomplete handwritten XML seed.

The initial device name uses the selected environment's authoritative Cloudflare
zone (the same Zone ID as DDNS), not the UI namespace. Derive the production name
from host identity and zone; disposable names use their recorded instance identity.
Keep resolved names in encrypted runtime delivery, never in public recipes.

For an identity-protected private backend, initialize `gui.insecureAdminAccess`
to acknowledge external proxy authentication and suppress the open-GUI banner.
Retain CSRF and the proxy/backend boundary. This setting can still produce a
Syncthing startup log warning. Do not clear a retained native GUI password.

An explicitly authorized reset backs up only the XML and preserves certificate,
private key, database and synchronized data. The initializer reuses retained
identity keys. Bootstrap implementation/acceptance is tracked in
[the initial configuration plan](../plan/SYNCTHING-INITIAL-CONFIG.md).
