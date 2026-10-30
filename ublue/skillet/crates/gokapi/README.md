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
  public origin, without the private UI prefix. Its root shows a brief sharing
  message; administrative endpoints return 403. Homer includes both links in Files.

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

## Disposable backup/restore acceptance

Run `integration_tests/gokapi-backup-restore.py` inside a provisioned, owned test
VM with Gokapi and its backup timer active. From `ublue/skillet`, upload through
`guest_command`, then invoke it through the same owner:

```sh
cargo run -p skillet_vm --example guest_command -- \
  "$(realpath ../butane)" donbot joplin --upload \
  "$PWD/integration_tests/gokapi-backup-restore.py" /var/tmp/gokapi-backup-restore.py
cargo run -p skillet_vm --example guest_command -- \
  "$(realpath ../butane)" donbot joplin /usr/bin/sudo -n /usr/bin/python3 \
  /var/tmp/gokapi-backup-restore.py --host donbot \
  --guest-hostname donbot-test-joplin --state /var/lib/data/gokapi \
  --backups /var/lib/data/syncthing/data/backups/gokapi
```

Select the actual host/instance, recorded guest hostname and declared paths.
This temporarily changes native accounts, configuration and a synthetic data
file. It pauses the timer, saves an independent original archive, exercises the
real backup unit and restore command, verifies rollback and native readiness,
then restores the original state and timer. It retains a root-only safety copy
at the printed guest path. It does not test browser authentication or public
sharing and does not run in routine CI.
