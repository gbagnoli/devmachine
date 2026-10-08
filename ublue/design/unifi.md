# UniFi controller

Run the self-hosted UniFi Network application as a rootful Podman container
with host networking, required for device discovery/adoption, and persistent
data under `/var/lib/data/unifi`. Preserve the current controller state when
migrating; ordinary Skillet apply must not overwrite or import a database.
The container runs as image user `unifi` (UID/GID 999). Skillet sets only the
declared bind-mount root to those numeric IDs and preserves ownership of
existing descendants; the host does not need matching account names. The
ownership and mode are typed volume inputs, so generic Podman code does not
infer identities from container configuration or recursively alter data.

Use UniFi Site Manager at `unifi.ui.com` for remote access. The cloud service
brokers access to the local controller; configuration and device management
remain on the server. Do not add UniFi to Caddy's private UI routes. Keep the
controller's local HTTPS management endpoint available on the host network.

Rupik is ARM64 and runs UniFi 10.0.162; clamps is x86_64. Validate a supported
application backup restore across architectures in an isolated VM before
production migration. The restored test copy must be fenced from production devices and cloud
services, including across reboot; NAT is not sufficient isolation. Validate
configuration locally. Site Manager and device connectivity belong to cutover
acceptance, when the original controller is stopped. Retain the old controller
and an independent backup until clamps manages the devices successfully.

See the [implementation plan](../plan/CLAMPS-UNIFI.md).
