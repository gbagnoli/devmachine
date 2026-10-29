# Calculon → donbot: source inventory

Status: source review completed 2026-10-11. This inventories declarations, not
running services. Private Chef overrides, deployed image versions, effective
nginx configuration and live storage have not been inspected. A bounded local
service-status check on 2026-10-16 confirmed active generated Quadlet services
for Prowlarr, Radarr, Sonarr, Lidarr and putioarr; this is not effective-config
or end-to-end acceptance. `systemctl cat` exposes generated units without sudo;
direct application configuration files remain unreadable to the agent. The target name
is **donbot**. Implementation is tracked in [the migration plan](DONBOT-MIGRATION.md).
Joplin and Airtrail are required migration targets; their access policies and
restore details still require effective inventory.
No literal domains, addresses, zone IDs or credential values belong here.

## Source and scope

Entry points: [role](../../roles/calculon.rb),
[default recipe](../../site-cookbooks/calculon/recipes/default.rb), and
[attributes](../../site-cookbooks/calculon/attributes/default.rb).
`default.rb` includes media, magiusstaff (which includes primary Syncthing),
Joplin, Airtrail and Gokapi, plus shared infrastructure and the excluded apps.
Several services require private attributes; inclusion alone does not establish
successful deployment. Default portal/media/notes domains are unset.

Reuse uCore/Skillet's existing OS, users, hardening, SSH/ET, Podman storage,
bridge DNS, Quadlets, Tailscale, Pi-hole, btrbk, Datadog and SMTP support. Do not
reimplement Chef repository/kernel installation, Rocky Btrfs packages, zram,
Podman engine setup, web pods, nginx/OAuth scaffolding or distrobox installation.
App data ownership, mount semantics and routing choices still require migration.

## Applications and access

Ports below are **current backend ports**, not a proposed public firewall list.
Paths beginning `/` in the access column are nginx portal routes. OAuth applies
to the portal, with dedicated exceptions described below.

| Service | Source image | Current access | Target access / scope |
| --- | --- | --- | --- |
| Prowlarr | `lscr.io/linuxserver/prowlarr:latest` | OAuth portal `/prowlarr`, 9696 | Verified tailnet-user authentication; no second UI login |
| Radarr | `lscr.io/linuxserver/radarr:latest` | OAuth portal `/radarr`, 7878 | Verified tailnet-user authentication; no second UI login |
| Sonarr | `lscr.io/linuxserver/sonarr:latest` | OAuth portal `/sonarr`, 8989 | Verified tailnet-user authentication; no second UI login |
| Lidarr | `lscr.io/linuxserver/lidarr:nightly` | OAuth portal `/lidarr`, 8686 | Tailnet-only UI; select release channel for fresh installation |
| putioarr | `ghcr.io/wouterdebie/putioarr:latest` | Shared web pod, 9091; no declared nginx UI route | Internal download integration for Arr; no public UI needed |
| Jellyfin | `lscr.io/linuxserver/jellyfin:latest` | Dedicated public HTTPS vhost when media domain is set, backend 8096, no OAuth | Public HTTPS, native application authentication |
| Plex | `docker.io/plexinc/pms-docker:latest` | Direct dual-stack host ports; **no Plex nginx vhost in this cookbook** | Public HTTPS; native Plex authentication and client discovery |
| Main Syncthing | `docker.io/syncthing/syncthing:latest` | OAuth portal `/sync`; GUI 8384, peer TCP/UDP 22000 | Tailnet-only GUI; peer connectivity is separate |
| Main Filebrowser | `docker.io/filebrowser/filebrowser:latest` | OAuth portal `/files`; backend 8080 via bridge-gateway port 8385 | Shared file-browsing UI on Syncthing hosts, tailnet-only |
| Personal Syncthing | Same Syncthing image | Login-user home state; private GUI gateway port 8388 and peer TCP/UDP 22202 | Retained as Donbot's explicit personal instance, with a private UI and no shared browser access |
| Tenant Syncthing | Same Syncthing image | Separate OAuth portal `/sync`; GUI gateway port 8386, peer TCP/UDP 22200 → 22000 | Deferred: preserve source data; no second instance in this migration |
| Tenant Filebrowser | Same Filebrowser image | Separate OAuth portal `/files`; backend 8080 via gateway port 8387 | Deferred alongside tenant Syncthing |
| Joplin Server | `docker.io/joplin/server:latest` | Backend 22300: public dedicated notes vhost **if configured**, otherwise OAuth portal `/joplin` | Proposed tailnet-only HTTPS with native Joplin authentication; verify mobile sync |
| Airtrail | `docker.io/johly/airtrail:latest` | Dedicated vhost, backend 3214: OAuth **except `/api/*`** | Tailnet-user authentication if supported; verify account integration and any required public API |
| Gokapi | `docker.io/f0rc3/gokapi` (implicit latest) | Backend 53842: OAuth admin plus public downloads/uploads/assets | Mixed: private administration, explicit public sharing routes |
| PostgreSQL: Joplin | `docker.io/postgres:18` resource default | Shared web pod, 5959; app connects over loopback | Internal named-container connection, no host publication |
| PostgreSQL: Airtrail | Same resource default | Shared web pod, 5858; app connects over loopback | Internal named-container connection, no host publication |
| Pi-hole | `docker.io/pihole/pihole:latest` default | Included through Tailscale recipe; web-pod backend 8088 by default, no calculon-specific UI vhost | Shared selected DNS service and private UI |
| Pi-hole synchronization | `ghcr.io/lovelaze/nebula-sync:latest` | Conditional hourly oneshot, only with enabled primary/replica/password settings | Inventory whether active; declare explicitly if retained |

Sources: [media](../../site-cookbooks/calculon/recipes/media.rb),
[Syncthing/Filebrowser](../../site-cookbooks/calculon/recipes/syncthing.rb),
[tenant](../../site-cookbooks/calculon/recipes/magiusstaff.rb),
[Joplin](../../site-cookbooks/calculon/recipes/joplin.rb),
[Airtrail](../../site-cookbooks/calculon/recipes/airtrail.rb),
[Gokapi](../../site-cookbooks/calculon/recipes/gokapi.rb), and
[PostgreSQL resource](../../site-cookbooks/calculon/resources/postgresql.rb),
[Pi-hole](../../site-cookbooks/pihole/recipes/default.rb), and
[conditional Pi-hole sync](../../site-cookbooks/pihole/recipes/nebula_sync.rb).

The tenant recipe logs “Skipping” for absent settings but does not return.
Check whether it is actually active; do not treat that log as evidence of absence.
The static portal also contains external links; a link is not another proxied app.
`mybibliotheca` has a path attribute but no application definition in this cookbook.
A Google Photos Takeout Helper binary is installed by the Syncthing recipe;
it is retained as a manual Donbot image utility, not a web service. The personal
`calculon::syncthing_user` instance is also retained separately from the deferred
magiusstaff tenant; target status is in the migration plan. An rclone template exists without a
recipe reference. Inventory manual uses before deciding either utility's fate.

## Arr installation scope

Donbot's Arr stack is a fresh installation, not an import of Calculon's app state.
Generate new internal API keys and deploy linked configurations. Use this source
inventory as a capability/schema reference only. The operator will move selected
media files after the new installation works; no legacy Arr database, watchlist
or Chef-key import is required.

## Confirmed putioarr/Lidarr capability

The operator confirms putioarr already works with Lidarr on calculon. The Chef
media recipe configures Sonarr, Radarr and Lidarr callbacks with separate API
keys and shared-pod loopback URLs/application subpaths. Target composition must
preserve all three and use container DNS in place of pod loopback. The recipe
also documents a possible manual putioarr binary override. Read-only
`systemctl cat` inspection on 2026-10-16 shows putioarr using its upstream
`:latest` image with config/download mounts and no binary bind override. Lidarr
uses `:nightly`, mounting its config, synchronized music and shared downloads.
The running image digest and binary version remain unverified; capture them
before choosing the target build. Do not infer absent
support from omission in the upstream README. Live key matching/download-client
settings were not inspected: root-owned files are unreadable to the agent and
noninteractive sudo requires a password.

## Persistent state and jobs

All paths below are **Chef defaults**. Confirm overrides, subvolumes, sizes,
ownership, ACLs, labels and application paths before any copy.

| State | Current default under `/var/lib/data` | Migration consequence |
| --- | --- | --- |
| Arr configuration | `prowlarr`, `radarr`, `sonarr`, `lidarr` → `/config` | Historical source only; fresh target configuration/new keys, no database import |
| Download integration | `putioarr` → `/config`; `media/downloads` → `/downloads` | Reference for integration schema; generate new internal credentials/API keys and deliver required external account credentials separately |
| Media library | `media/{movies,series}/{downloads,library}`, additional yoga/pilates library | Downloads bind-mounted into movie/series trees; do not copy bind views twice |
| Music | `sync/music`; Lidarr `/music`, media servers `/data/music` | Preserve shared ownership; cross-subvolume moves cannot be assumed atomic/hardlinked |
| Jellyfin | `jellyfin` → `/config` | Accounts, library metadata and settings; series mount currently has `/dara/tvshows` typo—inspect stored library paths before correcting |
| Plex | `plex/config` → `/config`, `plex/transcode` → `/transcode` | Preserve server identity/library state; transcode contents are temporary |
| Main synchronization | `sync` → `/var/syncthing`; `.filebrowser` holds DB/settings | Move to declared target layout explicitly; keep browser state outside synchronized user data |
| Tenant synchronization | `magiusstaff/sync`, its own `.filebrowser`, default UID/GID 2002 | Keep separate from primary media/data account (default 2001) and Syncthing identity |
| Gokapi | `gokapi/files` → `/app/data`, `gokapi/config` → `/app/config` | Files plus SQLite metadata/config, salts and any encryption material must be restored together |
| PostgreSQL | `dbs/{joplin,airtrail}` → `/var/lib/postgresql` | Confirm running major/layout; source image default 18 is not runtime evidence |
| Airtrail uploads | `sync/airtrail` → `/app/uploads` | Pair uploads with a consistent database backup |
| Proxy state | `www`: nginx configuration, certificates and caches | Import only required runtime values; regenerate Caddy configuration/certificates, discard cache contents |

Existing jobs:

- btrbk snapshots **only `sync` and `magiusstaff/sync`**, hourly; minimum 6h,
  retention `24h 31d 6m`. No whole-data snapshot policy. Existing shared btrbk
  supports caller-selected paths; preserve leaf-only selection.
- Daily PostgreSQL `pg_dumpall` compressed with zstd into
  `sync/backups/<app>`; prune archives older than seven days. Restore helpers
  exist. Atomic publication and coordination with uploaded files need review.
- Daily Airtrail uploads mirror into `sync/backups/airtrail/files`, with deletion
  and up to four hours jitter; its timer is independent of the database dump.
- Daily Arr cleanup deletes files/directories older than seven days from shared
  downloads. Replace only after defining how active/incomplete downloads are
  excluded; age alone does not establish completion.
- Registry auto-update comes from the shared Chef container resource. Record
  effective images/digests and choose compatible restore versions before enabling
  updates; do not upgrade application and database majors during the initial copy.

## nginx features to account for

### Jellyfin

The media vhost has **two server-side caches**, not just image headers:

- Images: `/Items/(.*)/Images`, 15 GB maximum, 30-day inactivity, revalidation
  and request locking.
- Video: regex under `/Videos/…`, 2 MB slices, 35,000 MB maximum, 90-day
  inactivity, 30-day validity for 200/206/301/302, stale responses on failures.
  A custom key includes selected transcoding parameters and byte slice.
- Browser cache headers vary by content type. There are `/` and `/web/`
  redirects, `/socket` WebSockets, a method filter, CSP/security headers and
  extension rejection rules.

The video cache ignores upstream `Expires`, `Cache-Control`, `Set-Cookie` and
`X-Accel-Expires`; its key contains no user/authentication identity. This is a
source finding, **not a demonstrated live exploit**. Any retained shared cache
must prove authorization and revocation still apply on cache hits. Cache-Control
headers alone do not reproduce these disk caches.

Caddy supports reverse proxying and WebSockets; Jellyfin recommends it for
HTTPS and requires correctly configured known proxies. See
[Jellyfin proxy guidance](https://jellyfin.org/docs/general/post-install/networking/reverse-proxy/)
and [Caddy reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).
Response caching needs an extra component:
[Caddy's cache module is nonstandard](https://caddyserver.com/docs/modules/http.handlers.cache),
with a [cache-handler implementation](https://github.com/caddyserver/cache-handler).
Neither source establishes parity with this nginx video slicing configuration.
Update, 2026-10-11: Caddy caching is selected for the target. Build and validate
`caddyserver/cache-handler` with a filesystem storage provider; see the
[Jellyfin cache plan](JELLYFIN-CADDY-CACHE.md). nginx video-slice parity remains
unproven and must be tested, not presumed from generic HTTP caching support.

### Plex

Chef publishes TCP 32400/8324/32469/1900 and UDP 32410–32414 on wildcard
IPv4/IPv6. Its firewall resource is named `syncthing`, also used by the actual
Syncthing recipe: inspect effective rules before relying on either declaration.
Discovery ports are not a proposed public streaming requirement.
Neither media container has a GPU mapping in the source.

Plex supports custom server access URLs for reverse proxies. A proposed Caddy
endpoint should be advertised explicitly as `https://<plex-hostname>:443`:
omitting the port can make Plex use its configured Remote Access port instead.
Require client authentication; never add the proxy bridge to Plex's
“allowed without auth” list. Prove secure **direct** playback on actual clients,
not fallback through Plex Relay. See [Plex network settings](https://support.plex.tv/articles/200430283-network/).

### Gokapi and Airtrail

Gokapi skips OAuth for `/api/uploadrequest/chunk/…`, assets/css/js/static,
favicon/manifest, `/publicUpload…`, `/downloadFile…`, and broad `^/d.*` routes.
It permits 200 MB requests with 300-second proxy timeouts. The app trusts
`X-Forwarded-Email` (`Authentication.Method=2`); removing OAuth requires an
explicit replacement for that identity contract. Its config template uses
version 22 and salt fields that are not supplied by the recipe's template
variables. Restore effective state instead of regenerating it from that template.

Derive an exact path/method list from the selected Gokapi version; retain public
sharing and guest-upload flows while denying administration and administrative
APIs externally. Use the planned verified tailnet identity gate plus Gokapi header
authentication for private administration, without a second password prompt;
strip client-supplied identity headers. Confirm whether the live zone is actually
Cloudflare-proxied before retaining `GOKAPI_USE_CLOUDFLARE=true`.
[Official Gokapi setup](https://gokapi.readthedocs.io/en/latest/setup.html)
describes native and header authentication; it does not justify copying these
legacy broad exemptions into Caddy unchanged.

Airtrail exempts every `/api/*` request from OAuth. Its `ORIGIN`,
`SCRIPT_NAME=/airtrail`, and loopback-only app/DB settings need review when
moving to separate containers and a dedicated hostname. Establish which API
clients exist and whether application authentication protects those requests.

## Filebrowser selection

Target selection reconciled 2026-10-16: Mikochi replaces the legacy Filebrowser
for the primary Syncthing instance. Shared composition and verified tailnet-user
authorization are implemented for clamps/joey; donbot needs its host declaration
and acceptance, not another browser module or authentication system. No internal
password prompt is intended. Preserve old browser databases/configuration until
required permissions and served paths are explicitly migrated; do not assume a
compatible database import. See [file browsing](../design/file-browsing.md).

## Infrastructure retained through shared support

Pi-hole is included by `tailscale.rb`; Tailscale is host-networked and persists
under `tailscale`. Chef advertises container subnets and an exit node, accepts
routes, disables subnet SNAT and enables masquerading. Current shared Skillet
supports selected exit-node policy but **does not yet implement all those routing
preferences**. Reconcile needed routing through the shared routing work in
[joey's plan](JOEY-MIGRATION.md); keep values private.

Datadog checks include SSH/ET, Btrfs, process/network monitoring, nginx status
and both Filebrowser/Syncthing instances. Reuse the Agent; service modules own
new checks and test/lab alert exclusions. Preserve intended production location
through private configuration instead of copying old provider labels blindly.
The included Pi-hole cookbook can also run hourly Nebula Sync: DNS/DHCP and
other selected settings plus gravity data, with primary/replica credentials.
Its presence depends on private configuration; this behavior is not implemented
by the current Skillet Pi-hole module. Keep one declared synchronization owner
and disable production replication in smoke tests.

No calculon-specific DDNS updater was found; opt in only if public addressing
actually requires it. nginx's `cloudflare true` configures trusted-proxy support;
it does not establish that DNS records are proxied through Cloudflare.

## Deferred tenant

Decision, 2026-10-11: do not port magiusstaff now. Keep both tenant rows above
as source evidence; its accounts, data, peer identity and snapshots remain
outside donbot implementation. Do not extend Skillet for multiple Syncthing
instances to satisfy this migration, or delete the source tenant state.

## Explicit exclusions

Do not migrate AdventureLog (frontend, backend and its PostGIS database/uploads),
Wanderer (web, PocketBase database and Meilisearch), or Openclaw and their ingress,
credentials and scheduled jobs. This does not authorize deleting source data or
removing shared dependencies needed by retained services.
