# PostgreSQL

Reusable rootful Podman composition for application-dedicated PostgreSQL
instances. The caller supplies the application identity, database/user names,
image, Btrfs data path, matching container data path, Podman network and
password. PostgreSQL has no host port; the caller connects over the declared
bridge network. See the [backup design](../../../design/postgres-app-backups.md).

Use a separate instance and password for each application. The official image
creates the configured `POSTGRES_USER` as a superuser, so this module must not
be used to share one instance between unrelated apps. Match the container data
mount to the selected image major: PostgreSQL 18 uses `/var/lib/postgresql`;
PostgreSQL 17 and older use `/var/lib/postgresql/data`.

The shared backup module installs a daily systemd timer for each caller-selected
instance. It runs `pg_dump --format=custom`, writes a private temporary archive,
then atomically publishes it to the caller's directory. The newest seven
archives are retained. Raw database files remain outside Syncthing. The caller
selects the backup directory under its Syncthing tree and the Syncthing owner;
the timer publishes files with that ownership.

On the guest, run the rootful Podman operations with
`sudo /var/usrlocal/bin/skillet-<host> postgres backup --instance <name>` to
create an immediate archive. Restore with
`sudo /var/usrlocal/bin/skillet-<host> postgres restore --instance <name> --archive <path>`. Restore
creates a fresh pre-restore archive, stops the caller's app unit, applies
`pg_restore --clean --if-exists`, then restarts the app if it had been active.
Use an archive from the same app instance and a compatible PostgreSQL major.
