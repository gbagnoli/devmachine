# Home Assistant

Proposed deployment/backup decision. Implementation belongs to
[the Home Assistant plan](../plan/HOME-ASSISTANT.md); this is not a deployed service.

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
Skillet manages deployment and explicitly owned initial settings; Home Assistant
owns its accounts, integrations, automations and internal configuration.

Expose `homeassistant.<host>.<ui-domain>` through the existing tailnet-restricted
Caddy/Cloudflare DNS-01 path. Add a shared host-upstream representation because
Caddy uses bridge networking and cannot resolve a host-network application as a
bridge container. Validate the Podman host gateway, proxy trust and listener
isolation together. Raw port 8123 must not bypass the private UI policy.
Retain Home Assistant login; tailnet authorization does not create an application
session. Machine A/AAAA records and UI CNAMEs remain unchanged; Pi-hole local
records and MagicDNS are not dependencies of this route.

## Backup and recovery

Use Home Assistant's scheduled native backups, with encryption enabled. The
Container local directory is `/config/backups`. Publish completed encrypted
archives to `/var/lib/data/syncthing/backups/homeassistant` using a bounded,
retryable exporter and atomic replacement in the destination filesystem. Keep
unfinished archives outside the synchronized namespace. This separates live
SQLite/configuration writes from replication and prevents partial backups from
becoming advertised recovery copies.

Explicitly share the backups folder in Syncthing. Prepare typed ownership/ACLs
for its mapped identity; never recursively change the live application tree.
The destination is an ordinary directory within the existing Syncthing leaf
subvolume. Clamps' existing btrbk selection snapshots it without adding the
Home Assistant state tree or all of `/var/lib/data` to snapshot policy.

Syncthing provides off-machine copies; btrbk provides local history. The latter
shares the source disk's failure domain, and synchronization propagates deletions.
Keep independent retention at the receiving site. Retain the backup encryption
key/emergency kit in the portable KeePassXC vault with an independent recovery
copy. Export original encrypted files, not the UI download that decrypts them.
Prove restoration into an isolated disposable instance before relying on backups.
Never run a restored production automation/integration against live devices.

References: [Container installation](https://www.home-assistant.io/installation/linux/),
[backup integration](https://www.home-assistant.io/integrations/backup/),
[backup and restore](https://www.home-assistant.io/common-tasks/general/),
[Container backup directory](https://github.com/home-assistant/core/blob/2026.10.1/homeassistant/components/backup/backup.py),
[HTTP/proxy configuration](https://www.home-assistant.io/integrations/http/).
