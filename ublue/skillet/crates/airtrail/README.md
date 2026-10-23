# AirTrail

Donbot composes AirTrail with its own PostgreSQL 18 database and a persistent
uploads directory. The private UI is routed through Caddy and appears in Homer.
AirTrail keeps native accounts; the Tailnet identity gate only controls network
access to the UI. Its database URL and optional provider API keys are delivered
through Podman secrets, not Quadlet environment values.

The scheduled daily backup stops AirTrail briefly and publishes one paired
backup directory containing a custom-format PostgreSQL dump and a copy of the
uploads. It retains the newest seven sets under the selected Syncthing backup
path. Restore creates a paired pre-restore recovery set, restores both parts,
and attempts to roll both back if database restore fails. Backups are included
in the host's selected Syncthing Btrfs snapshots.

On the guest, create a backup with `sudo /var/usrlocal/bin/skillet-donbot
airtrail backup`. Restore a selected set with
`sudo /var/usrlocal/bin/skillet-donbot airtrail restore --archive
<backup-set-path>`. Keep the matching application and database image versions
when restoring.

Optional integration entries are documented in the repository
[secret checklist](../../../../SECRETS.md). Smoke VMs never receive provider
keys. See [the Donbot plan](../../../../plan/DONBOT-MIGRATION.md) for remaining
client and paired restore acceptance.

AirTrail's upstream deployment and environment reference:
[Docker installation](https://github.com/johanohly/AirTrail/blob/main/docs/content/docs/install/docker-compose.mdx),
[environment variables](https://github.com/johanohly/AirTrail/blob/main/docs/content/docs/administration/environment-variables.mdx),
[AeroDataBox](https://github.com/johanohly/AirTrail/blob/main/docs/content/docs/integrations/aerodatabox.mdx),
[OpenAIP](https://github.com/johanohly/AirTrail/blob/main/docs/content/docs/integrations/openaip.mdx).
