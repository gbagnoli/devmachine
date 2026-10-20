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

The crate currently provides container composition but is not yet selected by a
host profile. Vault delivery, the Donbot caller, scheduled logical backup and
disposable restore acceptance remain to be implemented.
