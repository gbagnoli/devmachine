# Linked Arr services

Donbot selects Prowlarr, Sonarr, Radarr and Lidarr, with putioarr and SABnzbd
as independently selected download backends in separate rootful containers. Their shared media bridge supplies container DNS for API calls;
each browser UI also has its own Caddy backend network and the verified Tailnet
user gate. No backend port is published. Supported `External` authentication
removes the second browser login; machine API keys remain enabled.

Workstation provisioning creates independent API keys and the Transmission
password in the environment/host/instance vault scope before delivery. The
Put.io token is an explicitly supplied environment-specific leaf. Disposable
machines never inherit production provider credentials. Putioarr starts its
worker after validating the account, so live smoke acceptance requires an empty
dedicated test account, not the production token.

Fresh Arr `config.xml` files are atomically seeded before startup. Reapply
preserves application edits and refuses a vault/API-key mismatch. Putioarr gets
its immutable configuration as a Podman secret and runs its binary directly,
avoiding image initialization rewriting that secret. None of the payload enters
Quadlet environment literals, recordings or workstation plaintext files.

The guest reconciles through versioned consumer APIs and their published
schemas. Reserved `Skillet …` link names identify owned objects; duplicates or
different implementations fail. Only declared fields change; unrelated links,
libraries and settings survive. Prowlarr owns full synchronization of its
indexers into the applications. Skillet owns the application links and the
selected download-client links, not indexer accounts. Consumer connection tests
run before writes, and ambiguous writes are recovered by listing owned objects.
Existing IDs are retained during those tests. Servarr masks persisted secrets;
Skillet verifies the masked stored credential through the provider's connection
test before preserving it. A rejected credential is repaired only after the
expected vault value passes that same test.

Movies, TV and downloads use one `/data` bind so filesystem rename/hardlink
semantics can work. Music remains in the caller's Syncthing tree; cross-filesystem
imports may copy. No age-based download deletion is installed. Worker and
secret-file IDs resolve from
the caller's host data account; shared Syncthing directories retain that named
ownership rather than inheriting a container-image UID default.

SABnzbd is implemented as a paused, provider-free optional backend. Native INI
bootstrap seeds its key, exact UI hostnames and download categories only while
absent; later application edits survive. Its writable mounts contain only native
config and Usenet downloads. Native configuration is private state outside
Syncthing because it contains API keys and may later contain provider passwords.
The native local-range setting includes Tailnet addresses for forwarded browser
compatibility; Caddy's per-request identity decision remains the access boundary.

Arr application identity and download-backend selection are independent. New
media payloads use version 2; legacy Put.io-only payloads retain their wire shape.
The generated SABnzbd key also has its own encrypted credential: this records
that backend's initialization independently of the retained Arr bundle. Stage
both inputs before activation and reject a mismatched set. A missing key beside
its own native state or credential requires recovery, not an implicit rotation.

Prowlarr continues to own indexer accounts and sync; Skillet owns selected client
links. Provider delivery and actual NNTP download/import acceptance remain in
[the Usenet plan](../plan/DONBOT-USENET.md). Setup belongs in the
[media guide](../skillet/crates/media/README.md); remaining acceptance is in the
[Donbot plan](../plan/DONBOT-MIGRATION.md).
