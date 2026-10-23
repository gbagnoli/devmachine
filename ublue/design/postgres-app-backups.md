# PostgreSQL-backed application backups

Decision for Donbot: each application owns a separate PostgreSQL container,
database, credential and persistent data path. Shared Skillet PostgreSQL code
provides the common deployment and backup mechanics; applications do not share
a database container or superuser credential. This matches Calculon's existing
Joplin and AirTrail isolation while allowing the same module to serve both.

Joplin keeps its default database content store. Notes, attachments, accounts
and server metadata are therefore covered by a PostgreSQL logical dump. Use a
single-database custom-format `pg_dump` archive and restore it with `pg_restore`
into a compatible PostgreSQL major version. Joplin's client-side JEX export is
useful as a user-level export, but does not replace a server database backup.
If Joplin later enables filesystem or object storage, that content store must be
backed up and restored with the database from a consistent point.

Completed archives go under the caller-selected Syncthing backup directory;
raw PostgreSQL data stays outside Syncthing. Preserve the current daily,
approximately seven-day retention policy, but publish each archive atomically
so an interrupted dump cannot appear complete. A restore rehearsal must use
disposable state and the matching application/database versions.

AirTrail uses the same PostgreSQL logical-backup mechanism. Its upload directory
is additional application state and is captured with the database in one
coordinated restore set. The app stops briefly while both parts are copied; the
set is published atomically only after both are complete. Restore first creates
a paired recovery set, then replaces both database and uploads. Independent
database and file timers are not a consistency guarantee.
