# Joplin Server

Joplin Server container composition over a dedicated PostgreSQL instance. The
caller supplies its canonical HTTPS base URL, Podman network, application image,
database path and database credential. No host port is published; Caddy serves
the declared upstream on port 22300.

Joplin's default content store is PostgreSQL. Keep it enabled so one database
logical backup covers notes, attachments and server metadata. A client-side JEX
export is supplementary, not a server restore. If a filesystem or object store
is enabled later, include it in the same consistent backup set. See the
[backup design](../../../design/postgres-app-backups.md).

The caller declares Joplin as a UI service. Shared Caddy provisioning derives
its canonical hostname, and Homer includes it automatically. Native Joplin
login remains enabled. Donbot selects public `https://notes.<zone>/`, independently
of the private UI prefix, and sets `APP_BASE_URL` to that canonical URL. Public
delivery requires an explicit DNS destination; see [Caddy ingress](../caddy/README.md#public-services).
Retained private credentials preserve the old private route until redelivery.
Application account setup stays in Joplin; change default admin credentials before
making the service public. Skillet
use-or-creates the dedicated database password at
`skillet/environments/prod/hosts/<host>/joplin/database-password`; disposable
VM credentials are under that instance's test path. The password is delivered
to a dedicated PostgreSQL container and Joplin's Quadlet as a Podman secret.

The caller selects the backup path under its Syncthing data tree. Skillet
installs a daily timer, writes a custom-format `pg_dump` archive atomically,
and keeps the newest seven. Backups are owned by the selected Syncthing user.
On the host, run the rootful Podman operations with `sudo
/var/usrlocal/bin/skillet-<host> postgres backup --instance joplin`. Restore a
selected archive with `sudo /var/usrlocal/bin/skillet-<host> postgres restore
--instance joplin --archive
/var/lib/data/syncthing/data/backups/postgres/joplin/<archive>.dump`.
Restore stops Joplin, replaces database objects from that archive, and restarts
Joplin if it was active. Confirm the database image major is compatible before
restoring. A restore also retains a fresh pre-restore archive.
