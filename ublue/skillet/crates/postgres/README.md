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

The module currently converges the database container only. Scheduled logical
backup and restore support is planned and is not yet part of the module.
