# UniFi container on clamps

Status: implementation and empty-controller acceptance passed on the
disposable x86_64 VM on 2026-10-03. Backup restore and production migration
remain pending.

## Current behavior

Chef recipe: `site-cookbooks/rupik/recipes/unifi.rb`.

- Rootful Podman, `Network=host`, image `docker.io/jacobalberty/unifi:latest`,
  `User=unifi`, `TZ=Europe/Madrid`, `Restart=always`, network-online ordering,
  and registry auto-update.
- Rupik is ARM64; the running image package is
  `10.0.162-32076-1`. Clamps is x86_64.
- `/srv/unifi` is bind-mounted to `/unifi`, currently 1.2 GiB on Btrfs. Data
  files are owned by 999:999. The image user is UID/GID 999; Chef's host
  account `unifi` UID/GID 2666 is not the data owner. `/unifi/run` is a
  separate generated Podman volume.
- Active listeners: TCP 8080, 8443, 6789, 8843, 8880; UDP 3478, 10001. Local
  HTTPS on 8443 returned 302. The Chef repo has no UniFi nginx route.
- Eight small `.unf` backups were listed through October 2026. Restore validity
  and off-host availability are unverified.

## Scope and implementation

Implemented: reusable `skillet-unifi` module, called by clamps full apply. It
uses rootful Podman with mandatory host networking for device discovery and
adoption, `/var/lib/data/unifi` mounted at `/unifi`, the Chef image family,
`Europe/Madrid`, restart-at-boot, and registry auto-update. It requires the
shared data mount and prepares the service subvolume. The image's `unifi` user
owns the application state as UID/GID 999; Skillet sets those numeric IDs on
only the subvolume root and preserves existing application-owned files. The
host does not need matching account names. Rupik UID 999 is
`systemd-coredump`, not Chef's separate UID 2666 `unifi` account.

Do not add a Caddy route or a new credential flow for UniFi. UniFi Site Manager
at `unifi.ui.com` provides remote access to a locally running controller. The
control plane, configuration, and device management remain on the server; local
management remains available on the host network. Remote access must be enabled
for the controller's UI account.

## Migration and acceptance

1. **Passed:** Test the Quadlet/module on a named disposable x86_64 VM with an
   empty controller: service starts, remains active, uses intended ownership,
   has `Network=host` for discovery/adoption, and survives repeated apply.
   See `butane/ACCEPTANCE.md`. Reboot survival, expected host listeners, and
   confirmation that no Caddy/Cloudflare setup is needed remain to verify.
2. Test restoring one private rupik `.unf` backup into an isolated x86_64 VM
   using a compatible application version. Keep it disconnected from production
   devices. Confirm the restored site/configuration and remote Site Manager
   access without publishing backup contents or credentials.
3. Before cutover, restore the selected backup on clamps, verify remote access
   and device connectivity, and retain rupik plus an independent backup for
   rollback. Do not run two controllers against the same devices concurrently.

Add routine CI coverage for caller opt-in, Quadlet rendering, data-mount
dependency, and idempotent state. This coverage and the disposable VM runtime
acceptance are pending. Record commands and results in `butane/ACCEPTANCE.md`.
Device migration stays unverified until a backup restore or deliberate
re-adoption succeeds.

Read `skillet/AGENTS.md`, `design/storage.md`, `design/secrets.md`, and
`design/smoke-vms.md` before implementation.

References: [UniFi cloud architecture](https://help.ui.com/hc/en-us/articles/30319146145175-Understanding-UniFi-Cloud-Architecture),
[remote access](https://help.ui.com/hc/en-us/articles/360012192813-Introduction-to-UniFi),
[local management](https://help.ui.com/hc/en-us/articles/28457353760919-UniFi-Local-Management).
