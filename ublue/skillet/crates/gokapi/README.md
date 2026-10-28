# Gokapi

Donbot selects `f0rc3/gokapi:v2.2.4`. It runs rootful with the shared data
UID/GID, on its own private proxy network, with no published backend port.
Upgrade the pinned image together with its tested public route policy. Only
the dedicated proxy network is trusted for forwarded client addresses; automatic
trust of other container networks and Cloudflare-specific headers is disabled.
Initialization regenerates `/run/skillet/gokapi/proxy.env` from the actual
Podman network before every container start, including after reboot.

- Administration: `https://gokapi.<host>.<ui-domain>/admin`.
- Sharing: `https://send.<zone>/`; share links and guest-upload links use this
  public origin, without the private UI prefix. Its root and administrative
  endpoints intentionally return 403.

Add `skillet/environments/prod/hosts/<host>/gokapi/admin-user` in KeePassXC:
**Password** contains one complete lower-case LoginName from
`skillet/tailscale/ui-users`. Use `test` instead of `prod` for smoke provisioning.
This selects the initial super-admin explicitly. Other allowlisted users are
created with User rank and permission to create guest-upload requests; existing
accounts and manually assigned permissions are preserved. Unknown users are
never automatically registered. No application password/API key is needed in
the vault. Headscale identities must match the complete configured LoginName.

Use ordinary host apply, or stage only this credential:

```sh
cargo run --release -p skillet -- secrets deliver donbot gokapi
```

Deliver current Caddy configuration before activating the Gokapi phase. Native
setup runs in a temporary network-isolated container before the normal service.
Gokapi generates its own database, salts and account records. Repeated runs
preserve settings; interrupted setup resumes account creation. Changing the
initial admin or public URL requires explicit native reconfiguration; apply
refuses to silently rewrite ownership or security policy. The temporary unused
internal password is random, never logged or stored in KeePassXC.

The public proxy permits only pinned-version download/static routes and the
four guest-upload chunk endpoints. Native password cookies, request keys,
expiry and limits remain application-owned. Admin identity and Authorization
headers are removed publicly. Encoded paths and undeclared methods/routes are
rejected. Filename-in-URL mode is disabled initially; encoded filename paths
are deliberately outside this public contract. No sharing responses are cached.

## Backup and restore

Native state lives at `/var/lib/data/gokapi/{config,data}`. The daily
`skillet-gokapi-backup.timer` stops the service, publishes a paired state set
as tar files under `/var/lib/data/syncthing/data/backups/gokapi`, restarts it, and retains
seven completed sets. Temporary files use Syncthing’s excluded temporary-file
prefix, so incomplete archives are not replicated. Those files include SQLite, salts, API keys and any native
encryption material: retain the whole set with the uploaded files. Back up the
published directory using Syncthing/btrbk; never sync the live database.

On the guest:

```sh
sudo skillet gokapi backup
sudo skillet gokapi restore --archive /var/lib/data/syncthing/data/backups/gokapi/gokapi-TIMESTAMP.tar
```

Restore is explicit and stopped-service; it first saves a recovery set. It
rejects foreign manifests, symlinks and incomplete sets. Application state and
archives never belong in KeePassXC. Browser/Tailscale, real sharing limits and
full VM acceptance remain in the [Donbot plan](../../../plan/DONBOT-MIGRATION.md).

[Upstream setup](https://gokapi.readthedocs.io/en/latest/setup.html),
[pinned routes](https://github.com/Forceu/Gokapi/blob/v2.2.4/internal/webserver/Webserver.go).
