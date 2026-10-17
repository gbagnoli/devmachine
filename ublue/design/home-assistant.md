# Home Assistant

Deployment and backup decision. The optional module is implemented; production
activation and unverified acceptance remain in
[the Home Assistant plan](../plan/HOME-ASSISTANT.md).

## Deployment and access

Use Home Assistant Container as an optional shared Skillet service selected by
its host caller. Clamps is the first consumer; other hosts do not inherit it.
Run the official image as a rootful Quadlet with host networking for LAN
multicast discovery. Pin the application release and allow 60 seconds for clean
shutdown. Persist `/config` at `/var/lib/data/homeassistant/config` outside
Syncthing. Map only selected radios/devices and add Bluetooth D-Bus access when
required. Establish the effective capabilities and SELinux policy in runtime
acceptance rather than copying an unrestricted privileged example blindly.

Container installation has no Supervisor/app store. Companion services such as
MQTT and Zigbee2MQTT require their own declared containers and backup policies.
An offline invocation of the pinned image generates missing upstream config and
its includes. Its own HTTP store API initializes confirmed loopback/proxy settings
before publication; an unconfirmed YAML migration would otherwise revert after
five minutes. Existing app files and HTTP state are preserved. Skillet manages deployment and explicitly owned initial settings; Home Assistant
owns its accounts, integrations, automations and internal configuration.

Expose `homeassistant.<host>.<ui-domain>` through the existing tailnet-restricted
Caddy/Cloudflare DNS-01 path. The shared upstream transport distinguishes bridge
DNS from host loopback. Home Assistant binds 127.0.0.1; a dedicated confined Unix
relay gives bridged Caddy access without a bridge gateway or backend IP. A narrow
inet input rule blocks non-loopback TCP 8123 in both families independently of
application settings. Discovery remains host-networked. Caddy receives only the
relay socket, not host networking or the Tailscale daemon API.
Reuse verified tailnet-user authorization and retain Home Assistant login;
proxy identity does not create an application session. Validate companion app
login/refresh and WebSockets. No public webhook exception is selected. Machine A/AAAA records and UI CNAMEs remain unchanged; Pi-hole local
records and MagicDNS are not dependencies of this route.

## Backup and recovery

Use Home Assistant's scheduled native backups, with encryption enabled. A
post-onboarding administrator token enables API reconciliation; absent credentials
leave backup automation unconfigured without blocking the application. Persist
the generated encryption password in KeePassXC before encrypted delivery; do not
replace an existing native backup policy/key silently. The
Container local directory is `/config/backups`. Publish completed encrypted
archives to `/var/lib/data/syncthing/data/backups/homeassistant` using a bounded,
retryable exporter and atomic replacement in the destination filesystem. Keep
unfinished archives outside the synchronized namespace. This separates live
SQLite/configuration writes from replication and prevents partial backups from
becoming advertised recovery copies.

Place published backups under the caller-declared synchronized files path
(`syncthing/data` on clamps), and explicitly share that folder or include it in
an existing share. Prepare typed ownership/ACLs
for its mapped identity; never recursively change the live application tree.
The destination is an ordinary directory within the existing Syncthing leaf
subvolume. Clamps' existing btrbk selection snapshots it without adding the
Home Assistant state tree or all of `/var/lib/data` to snapshot policy.

Syncthing provides off-machine copies; btrbk provides local history. The latter
shares the source disk's failure domain, and synchronization propagates deletions.
Keep independent retention at the receiving site. Native local retention starts
at seven copies; published archives are initially retained without automatic
deletion, preserving recovery copies until an independent retention policy exists. Retain the backup encryption
key/emergency kit in the portable KeePassXC vault with an independent recovery
copy. Export original encrypted files, not the UI download that decrypts them.
Prove restoration into an isolated disposable instance before relying on backups.
Never run a restored production automation/integration against live devices.

References: [Container installation](https://www.home-assistant.io/installation/linux/),
[backup integration](https://www.home-assistant.io/integrations/backup/),
[backup and restore](https://www.home-assistant.io/common-tasks/general/),
[Container backup directory](https://github.com/home-assistant/core/blob/2026.10.1/homeassistant/components/backup/backup.py),
[HTTP/proxy configuration](https://www.home-assistant.io/integrations/http/).
