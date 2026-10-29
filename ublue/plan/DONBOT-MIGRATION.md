# Donbot fresh installation

Status: **Joplin and AirTrail implementation, named-VM provisioning, and
database restore acceptance are complete; client/API acceptance remains open**,
refreshed 2026-10-28. Jellyfin library access and static-cache consumer checks
passed; staging DNS-01 and VM-local HTTPS passed, with native client acceptance
open (slice 5). Gokapi composition, native account bootstrap, mixed ingress and
paired tar backups are implemented; desktop/Tailscale and full named-VM
acceptance are deferred until the operator has desktop access (slice 6).
Linked-media provisioning and declared-application
repeat/reboot acceptance also passed; transfer/import and operator Arr browser
checks remain open. Inventory is complete. Donbot
profile, binary, VM Butane template, Joplin and AirTrail callers, generated
KeePass credential delivery, Homer registration, and scheduled backups are
implemented. The fresh named VM currently runs both services and their dedicated
PostgreSQL containers; internal Joplin API ping and AirTrail root requests each
returned HTTP 200. Both backup timers are enabled. Joplin restore and AirTrail
paired database/uploads restore passed in the disposable VM, and the operator
confirmed both UIs work. Joplin mobile/desktop sync and AirTrail account/API
acceptance remain open. The VM's test Caddy configuration uses the ACME staging
CA, which browsers do not trust by default. The earlier Joplin-only VM was disposed through Skillet's
lifecycle command before this fresh run was created. Shared file browsing and identity
authorization are implemented for clamps/joey; operator browser acceptance
exists for clamps, not donbot. Encrypted Pi-hole v6 custom DNS delivery is also
implemented. Read
[DONBOT-INVENTORY.md](DONBOT-INVENTORY.md) before changing code. Follow the
[refactoring roadmap](SKILLET-REFACTOR.md) and keep its parked acceptance visible;
this plan does not close those gates or claim fresh donbot acceptance.

## Scope and prerequisites

Linked Arr implementation added 2026-10-26: Donbot selects all five media
containers, private UI/Homer declarations, generated-key delivery and API
reconciliation. Test credentials are now delivered and first full provisioning
passed. Declared-application reboot/reapply acceptance passed on 2026-10-27;
transfer/import and browser acceptance remain open;
there is no automatic production credential fallback. See slice 4 below.

Build a fresh Donbot installation with Arr/putioarr, Plex, Jellyfin, Gokapi,
Syncthing and a private file-browsing UI. **Joplin and Airtrail are required
services**; establish their effective access policies, client requirements and
restore inputs in slice 0. **Defer the magiusstaff tenant**, including its second
Syncthing/Filebrowser instance and snapshot selection. Do not add multi-instance
support. Leave existing source-host state untouched; this plan does not include
an application data or account migration. Exclude AdventureLog,
Wanderer and Openclaw.

Reuse the existing fleet baseline. Do not port Chef host/Podman setup. Only add
OS image packages where an actual service requires them; applications stay in
rootful containers. `ucore-images/recipes/ucore-donbot.yml` exists as a thin image;
the local Skillet profile/binary and VM-only Butane template now select it. Do not clone
clamps-specific service choices such as UniFi or Home Assistant.
The shared Homer dashboard is selected by the fleet baseline. Its generated
links now include every declared UI site, including Joplin.

Accepted access requirements: former OAuth-protected UIs become tailnet-only;
Joplin and Plex/Jellyfin require public HTTPS without a tailnet client; mixed services
keep explicitly required public flows. HTTPS is mandatory for all web endpoints.
Architecture constraints and proposals are in
[public service access](../design/public-service-access.md).

## Service gaps

| Service / capability | Missing for donbot |
| --- | --- |
| Existing baseline: Pi-hole, Tailscale, Syncthing, Mikochi, private Caddy, SMTP, Datadog, btrbk | Donbot declarations and shared composition are implemented; finish host-specific acceptance using existing modules |
| Subnet routing | Shared routing preferences tracked in joey's plan; select only needed routes |
| Prowlarr, Radarr, Sonarr, Lidarr, putioarr | Composition, mounts, credentials, private UIs/Homer, owned links and monitoring declarations implemented. Real connection tests and application reboot/reapply passed; transfer/import and operator browser checks remain. No age-based cleanup is enabled |
| Jellyfin and Plex | Service configuration/state restore, public HTTPS ingress, client acceptance; Jellyfin Caddy cache per its plan |
| Gokapi | Implemented private header-auth admin, separate public sharing, native accounts and paired tar backup/restore. Desktop/Tailscale and full VM acceptance deferred |
| Joplin / Airtrail (required) | Both callers, reusable PostgreSQL module, credentials, current private UI/Homer registration and scheduled backups are implemented. Database/upload restores passed. Joplin requires public HTTPS and external client sync acceptance; AirTrail remains private with account/API acceptance open |
| Conditional Pi-hole synchronization | Confirm current use; add explicit optional module only if retained; static DNS inventory is already shared |

Donbot baseline, Joplin and AirTrail are wired to the canonical host profile.
Joplin PostgreSQL and AirTrail paired database/uploads backup/restore acceptance
passed. The fresh named VM has both services running, with PostgreSQL and
scheduled backup timers active; the operator confirmed both UIs work. Joplin
client sync and AirTrail account/API acceptance remain open. Media and Gokapi
implementations have separate status: the linked media stack is implemented
with transfer/import and operator browser acceptance pending; Gokapi is implemented with desktop/Tailscale and full VM acceptance deferred.
Donbot does not select DDNS. Joplin requires public HTTPS with native authentication;
its current private route must change through the generic ingress milestone. Arr uses
verified tailnet-user authentication. Airtrail should use the same tailnet-user
authentication where supported; its account integration and API needs require inventory. UniFi, NUT and Home Assistant are not
part of this migration. Do not add them by copying another host's selections.

## Donbot baseline service selection

Declare these existing modules in the donbot caller:

- **Pi-hole:** DNS plus shared private-record inventory; identity-protected UI.
- **Syncthing:** persistent shared files and a private GUI.
- **Mikochi:** file browsing of the declared Syncthing files, included through
  shared composition; private UI with verified tailnet-user authentication.
- **Tailscale:** host-network access, private UI identity lookup and explicitly
  selected production exit-node advertisement. Subnet routing remains a separate
  policy; it is not implied by exit-node selection. Disposable VMs must not
  advertise a production exit node or production subnets.
- **Caddy and tailnet authorization:** trusted private HTTPS, declared aliases,
  Cloudflare DNS/certificate lifecycle and the shared identity gate.
- **Datadog:** shared Agent and service-owned checks, test/lab alert exclusions.
- **Postfix SMTP relay:** shared outbound email infrastructure.
- **btrbk:** snapshots of caller-selected leaves, initially Syncthing only;
  application backup artifacts may live there, raw databases do not.

Reuse OS/user/hardening, SSH/sudo/ET, Podman storage/networking and update timers.
These are host infrastructure, not additional application migrations. Select real
native networking and encryption profiles separately from VM configuration.
DDNS is excluded. UniFi, UPS and
Home Assistant are not part of donbot's baseline. Pi-hole settings replication
remains conditional on an explicit requirement, not on mere credential presence.

## Short execution order

The detailed slice numbers below identify work packages, not a requirement to
finish public ingress before private services. Use this dependency order:

1. **Donbot baseline and named VM (slices 0–1):** add host/profile/Butane and
   select existing fleet services. Boot a signed image and prove data persistence,
   private TLS UIs, identity authorization and repeated apply/reboot. Source
   inventory is bounded to information needed by the next service; do not block
   the fresh Arr installation on legacy database or key inspection.
2. **Fresh linked Arr stack (slice 4):** generate keys, configure Prowlarr,
   Sonarr/Radarr/Lidarr and putioarr automatically. Prove connection tests and a
   harmless import with no second browser login. Evaluate optional Usenet client
   separately; no provider purchase or transport removal is implied.
3. **Public/mixed ingress (slice 2), Joplin first:** extend shared typed Caddy/DNS
   policies and use the existing Joplin caller as the first public consumer.
   Public/private rendering, fixed names and explicit DNS destinations are
   implemented; finish live public DNS/ACME and internet acceptance.
   Prove public destinations, private denials and owned provider cleanup. Public
   Joplin keeps native authentication without a tailnet gate or proxy login.
   Separately prove real internet reachability; local VM forwarding is insufficient.
   Mixed route enforcement follows before Gokapi; this work also enables media sharing.
4. **Joplin client acceptance (slice 7):** verify a client with Tailscale
   disconnected can use trusted public HTTPS and complete real mobile/desktop
   note and attachment synchronization with native Joplin login.
   The named VM backup and restore round trip is already accepted.
5. **AirTrail acceptance (slice 7):** decide whether to create an application
   account and verify the supported client/API behavior. Paired DB/upload backup
   and restore passed on disposable state. Current implementation uses native
   AirTrail login behind the tailnet gate; optional provider integrations stay
   absent unless their vault entries are supplied.
6. **Jellyfin/Plex and cache (slice 5):** add service modules, selected media paths
   and public trusted HTTPS. Validate actual external playback, seek, subtitles and
   available transcoding. Follow the separate Caddy cache milestones; no assumed
   video slicing parity.
7. **Gokapi (slice 6):** configure private verified-user administration and explicit
   public download/guest-upload flows. Prove real external sharing, limits/expiry,
   private administration denials and coordinated files/metadata restore.
8. **Final rehearsal and hardware deployment (slice 8):** recreate the named VM,
   check backups and all selected services, then prepare the real disk/network
   profile. This is a fresh install: old Joplin/Airtrail/Gokapi accounts and data
   stay on Calculon. The operator may copy selected Arr media later, outside this
   provisioning work.

## Required app inputs and state

| App | Configuration / credentials | Persistent state and acceptance |
| --- | --- | --- |
| Joplin | Canonical HTTPS base URL, dedicated PostgreSQL credentials, native user/admin accounts; shared SMTP if mail is enabled | PostgreSQL notes/accounts/resources by default, plus any selected external content store; mobile/desktop sync, account isolation and logical backup/restore |
| Airtrail | Canonical origin, dedicated PostgreSQL credentials, supported account/auth integration; optional KeePassXC integration keys (AeroDataBox/OpenAIP) | PostgreSQL plus uploads as a consistent backup set; account ownership, attachments and required API flows |
| Gokapi | Public sharing/private admin URLs, verified identity-to-account roles, app-required signing/encryption secrets/settings | Uploaded files plus SQLite metadata/configuration and required encryption material; download/upload, expiry/limits, private-admin policy and complete restore |

Internal passwords/keys should use existing use-or-create delivery where the
application supports it. External account tokens remain operator-supplied.
For Airtrail's supported provider integrations (currently AeroDataBox/OpenAIP in
Chef), a present valid KeePassXC key enables that integration; an absent entry
omits its configuration and does not block provisioning. Handle each independently.
The Airtrail service itself is still explicitly selected by the host caller.
Present empty/malformed keys fail validation with redacted diagnostics rather
than silently disabling the integration. `secrets check` treats absent keys as
optional and validates present entries. Reconciliation removes previously managed
integration configuration when its entry is removed; test present/absent/removal
and repeated apply without exposing values.

Exact new vault entries belong in the shared secret catalog/root checklist when
implemented; do not invent mandatory provider credentials for disabled features.
Application account creation must use the selected version's supported mechanism.

All three need fresh-install acceptance even if existing data is later imported.
For data transfer, Joplin needs its chosen content store along with the database;
Airtrail needs DB and uploads together; Gokapi needs files, metadata and encryption
material together. Keep raw database state outside Syncthing; publish completed
consistent backup artifacts into its declared backup folder and snapshot only
caller-selected leaves. No live source state is altered by this plan.

## Implementation slices and exit criteria

Implement one slice at a time, with focused validated commits. Do not silently
turn the source inventory into an assumption about running state.

### 0. Complete the effective inventory

Read effective private Chef settings and live service/container/nginx definitions
without recording credentials. Record privately: image versions/digests,
application URLs and accounts, public IPv4/IPv6 and NAT/firewalls,
media sizes/mounts/subvolumes, hardware acceleration, backup/restore state, and
actual Jellyfin cache hits/load. Preserve the operator-confirmed working putioarr/Lidarr integration. Check
installed putioarr image/binary provenance, including any manual override noted
in Chef, and the Plex/Syncthing firewall resource collision.

**Exit:** each retained service has a version, state/mount map and access policy;
validate public Joplin HTTPS and native login against mobile/client needs,
Airtrail public API clients, conditional Pi-hole synchronization, source subvolumes inside
synchronized data, and public reachability.
Keep private evidence outside Git. No source service is stopped or altered.

### 1. Add donbot using existing baseline

Add the canonical host profile, thin host binary, Butane selector/install inputs
and matching image policy. Select existing Syncthing, host-network Tailscale,
btrbk, Datadog and SMTP; explicitly enable production exit-node advertisement.
Keep exit-node advertisement disabled in ordinary smoke provisioning. Include Pi-hole for the intended third resolver and
mask resolved in the host image if selected. Choose a data account mapping that
preserves the source's shared media permissions without assuming workstation IDs.
Keep `/dev/vda` VM inputs and physical disk/root-profile choices distinct.

**Exit:** profile drives credentials/UIs/monitoring/readiness; strict Butane and
local CI pass; a fresh named donbot VM reaches signed-image base readiness,
repeat apply and reboot preserve the shared data mount. No source peer identity,
production routes or public DNS writer is imported into a smoke VM.
Shared subnet routing work stays tracked in joey's plan. Reuse the implemented
encrypted Pi-hole v6 custom DNS inventory; do not add another delivery path. If live
Nebula Sync is needed, add it as an explicit shared capability with private
primary/replica delivery and a single production owner; smoke must not replicate
production settings or leases.

### 2. Generic ingress and verified tailnet authentication

Public/private policy and fixed zone-root names are implemented, with Joplin as
the first public caller. Version 6 binds the policy to the receiving caller;
matching retained private payloads preserve identity protection until redelivery.
The shared provider owner uses explicit public destinations alongside private
tailnet records. `--public-target` selects public IPs or an existing public DNS
hostname; no extra vault naming configuration is required. IPs produce a separate
`public.<host>.<zone>` A/AAAA target with service CNAMEs. Missing targets fail
before deployment/provider mutations. Mixed path/method policy remains pending.

Joplin is the first public consumer: select public DNS destinations,
trusted TLS and native application authentication without the tailnet gate or a
proxy login. Preserve `APP_BASE_URL`, sync methods/headers/bodies and private
database/backend ports. Keep its sync traffic uncached. Existing private routes
remain restricted until their caller explicitly selects public exposure.
VM provisioning additionally checks the public Host/SNI against Joplin's native
ping and protected API after Joplin activation; production probes check TLS trust
and staging probes explicitly relax only CA validation. This local guest probe
does not establish public internet reachability. Live public DNS/ACME acceptance
is pending an operator-provided destination and suitable edge routing.

Reuse the implemented caller-selected identity gate for file browsing; add the
Gokapi identity-to-account adapter for its admin flow, following [tailnet UI authentication](../design/tailnet-ui-authentication.md).
Resolve the actual connection peer through the existing host-network daemon
LocalAPI; validate user ownership and the explicit allowlist before upstream
access. Preserve Caddy-observed source addresses through rootful port forwarding.
Do not trust browser headers or grant every tailnet user administrative access.
Keep the allowlist/mapping in privately delivered configuration. The shared
Rust forward-auth gate now implements identity authorization for Mikochi. It
uses the existing daemon, without enrolling Caddy as another node. Expose only
the authorization endpoint to Caddy through a bounded trust boundary;
a read-only mount is not an API authorization policy. Validate the Headscale
LocalAPI identity path separately; current workstation enrollment is hosted
Tailscale-specific and this work does not claim Headscale lifecycle support.

Keep public machine address records separate from tailnet machine records;
CNAMEs/aliases must target the declared access destination. Reuse DNS-01 and
zone-scoped issuance/cleanup. Address updates and certificate renewal have
different owners: donbot does not run DDNS. Reuse existing shared provider
ownership checks without introducing an address updater.
Public sites use DNS-only records by default. Advertise AAAA only when routed
IPv6 has been verified. Public 443 must work with Tailscale disconnected; port
80 may redirect to HTTPS, but never serves an application in plaintext.

Derive public names from the environment's Cloudflare zone name, resolved using
its existing zone ID. Caller declarations supply fixed labels: Joplin `notes`,
Plex `plex`, Jellyfin `video` and `media`. No extra vault entries or configurable
per-app public labels are required now. These names omit the private UI prefix
and host label: private `ui.example.invalid` yields `notes.example.invalid`,
`plex.example.invalid`, `video.example.invalid` and `media.example.invalid`.
Reconcile DNS, TLS and proxy routing for every declared public alias using the
same deployment ownership guards. Preserve private UI naming independently.

**Exit:** CI verifies actual adapted Caddy routing with dummy names: public sites
reachable; private sites denied externally; mixed routes default-deny; aliases
have identical policy; unknown Host/SNI and spoofed identity/forwarded headers
cannot select a private upstream. Identity tests distinguish the two allowed
users, a denied user, a tagged device, unavailable WhoIs and altered headers;
Gokapi receives only verified identity and file browsing has no second login.
Use LocalAPI fixtures for CI, then an actual user-owned client for acceptance
(the tagged server VM cannot stand in for a human client). IPv4/IPv6 rules,
payload migration, DNS
ownership/retry/cleanup and credential redaction have tests. Named-VM HTTPS
acceptance covers both allowed and denied routes; separately prove real internet
reachability because passt/localhost testing cannot establish it.
Test public name derivation with a customized private UI prefix as well as the
default `ui`; neither prefix nor host identity may enter the public names. Both
Jellyfin names must have DNS, TLS coverage and matching upstream/access policy.

### 3. Validate reused private file browsing

Mikochi is selected for the current shared-root scope; Quantum and
Copyparty remain alternatives. See [file browsing](../design/file-browsing.md).
The shared module is implemented alongside Syncthing for clamps and joey.
Reuse canonical caller composition when donbot is added. Declare served folders,
read/write policy and data ownership. Keep app DB/config outside synchronized
user files, on persistent service storage; do not expose the entire data mount.
Use container DNS and a private Caddy hostname with no host GUI port.
**No internal username/password login:** reuse the verified tailnet-user policy
for the two intended users. For Mikochi, configure `NO_AUTH=true` only after
Caddy identity enforcement and backend isolation pass. A private IP-range
matcher alone does not satisfy this requirement. The existing shared authentication path is sufficient here; validate private
file browsing with the baseline before new public/mixed ingress work. Confine the unauthenticated
backend to a dedicated private network with Caddy as its only proxy client;
prove isolation from host/LAN/tailnet and other application networks rather than
assuming that omitted host ports provide it. No file-browser password entry is
required by this policy; shared identity configuration remains audited.

**Exit:** all declared Syncthing hosts select the same module; credentials and
monitoring follow that declaration. Tests cover mounting/ownership, absent
state, private ingress, duplicate names and repeat apply. A named VM can upload,
read and delete a sample file with expected permissions, persists settings after
reboot and denies an outside-tailnet request. Migration of existing browser
accounts/settings is explicit and the original DB is preserved. Both allowed
users enter without an app password; other users, tagged devices, anonymous
clients and direct backend connections are denied. A daemon outage fails closed.

### 4. Fresh Arr installation and download integration

Implementation added 2026-10-26: the cohesive media crate contains bootstrap,
private container composition, bounded HTTP clients and owned-link reconciliation.
Donbot caller, encrypted workstation/guest delivery, Homer and application-smoke
checks are wired. The provider-free named-VM consumer test passed with four
actual Arr images: API-key enforcement, external browser authentication,
Prowlarr connections, library roots, unchanged repeat apply and container restart
persistence. The test token is now present at
`skillet/environments/test/hosts/donbot/media/putio-token` (Password), copied
from production at the operator's explicit request. First full provisioning,
including real media link reconciliation, passed. Production uses its own
environment path; generated keys need no manual setup.
Architecture is documented in [media stack](../design/media-stack.md).
Declared-application reapply and reboot acceptance passed on 2026-10-27.
Remaining acceptance must prove a harmless transfer/import and its callbacks,
and authorized private browser access.
First apply exercised putioarr startup and all three Transmission connection
tests, separately from the earlier provider-free consumer test.

Donbot gets new Prowlarr/Sonarr/Radarr/Lidarr and putioarr configuration. No
legacy Arr databases, API keys, watchlists or download-client configuration are
imported. Calculon's setup is a reference for working capabilities and schema.
The operator will move selected media files after the new stack is accepted;
that later file transfer is not a prerequisite for service provisioning.

Use a cohesive media module with typed per-app configuration and shared helpers,
not a new broad system trait or a parallel host service registry. Declare
Prowlarr/Radarr/Sonarr/Lidarr and putioarr as separate bridge containers. Replace
pod loopback references with container DNS; prefer dedicated private hostnames
and configure fresh application URL bases.

Protect every Arr browser UI with the shared verified tailnet-user allowlist,
not a source-range check alone. Avoid a second application login using the
selected version's supported external-authentication mode. Verify that mode for
each application; do not weaken backend isolation to bypass enforced login.
Keep API keys for Arr-to-Arr and putioarr calls on private internal networks;
browser authentication does not replace machine API authentication.

Resolve Put.io credentials, putioarr password and Arr API keys through existing
templates/delivery. Avoid literal secret environments; use supported secret
files or existing secret-aware runtime configuration. Generate new application
keys once and reuse them on repeat apply. Review latest/nightly channels and
configuration compatibility before enabling automatic updates.

#### Reconcile application links automatically

Deployment must produce a connected stack, not just running containers. After
readiness, reconcile supported application APIs using Rust clients and internal
container DNS. Register Sonarr, Radarr and Lidarr in Prowlarr with their own API
keys and reachable application URLs; configure Prowlarr's URL for the indexers
it synchronizes back. Choose explicit indexer sync ownership/mode so Prowlarr
and Skillet do not overwrite one another's managed objects.

Configure each selected Arr download client with the correct endpoint, protocol,
credentials, category and completed-download paths. Putioarr presents a
Transmission-compatible endpoint; configure its required reverse application
references too. Lidarr integration already works on calculon, confirmed by the
operator; the Chef recipe declares `[lidarr]`, `[sonarr]` and `[radarr]` callbacks
with each application's API key. Preserve all three, replacing shared-pod loopback
URLs with container DNS and the selected application URL bases. A shorter upstream
README is not evidence of missing deployed support. Capture the installed
image/binary and effective schema before choosing the target build: Chef notes a
possible custom binary override. Preserve that capability and prove it again in
the replacement VM rather than treating Lidarr as an unsupported new integration.

Bootstrap API keys before reconciling links. Use-or-create a separate
cryptographically random key per
application, persist it in the correct vault scope before delivery, and seed the
consumer-supported bootstrap configuration before its first start. Verify the
selected image/version's configuration mechanism; current upstream reads `ApiKey`
from configuration and generates one when absent. Do not attempt initial key
installation through an API that already requires that key. Include Prowlarr's
own key as well as Sonarr/Radarr/Lidarr; the Chef callback keys alone do not cover
all integrations. Smoke keys are instance-scoped and never reuse production keys.

Start applications with private backends, verify authenticated readiness with
the selected keys, then reconcile Prowlarr application links, download clients
and putioarr callbacks. Resume after interruption with the same persisted keys.
Ordinary apply never rotates keys; rotation is a coordinated explicit
operation affecting the app and every consumer. Add generated requirements
to the shared secret catalog and checklist when implementing this slice.

Use existing generated/reused secret delivery for API keys. Reconcile only declared owned links by stable identity; avoid duplicates,
preserve unrelated settings/libraries, and recover from partial initialization.
Apply bootstrap file changes only through supported lifecycle boundaries, not
by overwriting live application databases/configuration.

**Exit:** real consumer connection tests pass for each declared application and
download client, including correct category/path mapping and a harmless import.
Reapply and interrupted-link recovery create no duplicates or rotate generated
keys. CI uses local API fixtures; named-VM checks exercise actual consumers.

#### Explore Usenet as an additional download backend

Keep putioarr selected until replacement is explicitly chosen. Evaluate
SABnzbd (proposed first trial) and NZBGet as separate optional caller-selected
backends. Usenet needs both a provider/server account and suitable NZB indexers;
Prowlarr manages indexers, the downloader handles provider credentials and NNTP
TLS. Arr applications submit downloads directly to the selected client.

Keep credentials in audited vault leaves, configuration in typed templates,
and downloader UI behind verified tailnet authentication with a private backend.
Configure distinct TV/movie/music categories and consistent shared download
paths. Preserve API authentication for internal calls. Account for incomplete
files, repair/unpack temporary space, failure retries and safe completed imports.
Test with provider-authorized sample data, not production watchlists. Do not add
provider credentials or new secret requirements until a backend is selected.

**Decision/exit:** select a client and provider/indexer inputs; demonstrate each
Arr's supported connection, successful download/repair/import and repeat
convergence. If both transports remain, declare protocol preferences explicitly;
do not imply that adding Usenet automatically replaces putioarr or provides
cross-protocol retry for every failed download. References:
[Prowlarr integration](https://wiki.servarr.com/en/prowlarr/quick-start-guide),
[putioarr](https://github.com/wouterdebie/putioarr),
[SABnzbd flow](https://sabnzbd.org/wiki/introduction/usage), and
[NZBGet integration](https://nzbget.com/documentation/how-to-link-nzbget-with-sonarr-radarr-and-other-tools/).

Declare media/download/library mounts and identity mappings once. Prefer common
container paths for downloads/imports, following the
[Servarr mount guidance](https://wiki.servarr.com/en/sonarr/installation/docker);
verify hardlinks/atomic rename on the
actual filesystem instead of assuming every Btrfs subvolume shares that behavior.
Preserve separate synchronized music policy explicitly. Replace the age-only
cleanup job with a confirmed completed-download policy or leave it disabled.

**Exit:** freshly bootstrapped applications can connect over bridge DNS, a harmless
sample transfer/import reaches the right library with correct ownership, private
UI routes work without a second login for allowed users; denied/tagged clients
and identity lookup failures are rejected. Internal API-key calls still work and
no backend UI port is published. Repeat apply/reboot preserve
app state; secrets are absent from Quadlets/recordings. Any path remapping and
copy-versus-hardlink behavior is documented.

### 5. Port Jellyfin, then Plex, with public HTTPS

Jellyfin module/caller, Homer link, read-only shared Arr library mounts, public
`video`/`media` DNS declarations and typed anonymous static caching are implemented
(2026-10-28). Publish the controlled Caddy service image before cache-enabled
provisioning. DNS/ACME activation requires an explicit destination. Test provisioning accepts
LAN/ULA addresses for VM-local or routed LAN checks; production IPs remain public.
The passt VM has no independent LAN address; mirrored host addresses must not be
published as if they belonged exclusively to the VM.
Native accounts/library selection, known-proxy configuration, real external
playback, backup/restore and GPU acceptance remain open. Plex now has a reusable
crate/caller, an encrypted Caddy-config apply phase, loopback-only setup ports,
shared read-only libraries, a public `plex` name and a Homer Media card.
Plex account claiming and secure direct-client playback remain operator gates.
Caddy returns 503 until the native claim marker exists; claim over the private
SSH tunnel and reapply Caddy. Staging DNS-01 and VM-local HTTPS/native-auth
checks passed for Joplin and both Jellyfin names. External LAN/internet access
and trusted-client playback remain unverified.
Protected artwork and video caching remain gated by the cache plan.

Use `video.<zone>` and `media.<zone>` for Jellyfin and `plex.<zone>` for Plex,
with Caddy TLS and named bridge upstreams. Keep native
application authentication; backend HTTP remains confined to the bridge.
Migrate Jellyfin's known-proxy/client-IP policy, library paths and WebSockets.
Inspect the legacy `/dara/tvshows` path before fixing it. Declare GPU access only
for hardware actually present; preserve a CPU-only configuration.

For Plex preserve server identity and library state. Publish the explicit custom
access URL `https://plex.<zone>:443`, and test intended Plex clients through
that route. Do not expose discovery ports publicly or grant unauthenticated
access to the proxy subnet. A native Plex TLS port is an alternative only if
client acceptance demonstrates a need and its security policy is reviewed.

**Exit:** a real client without Tailscale can authenticate, play, seek/range,
resume and load subtitles through trusted HTTPS on each service. Test direct
play and an available transcode path; Plex reports a secure direct connection,
not Relay fallback. Unauthenticated restricted APIs/media stay denied.
Unknown hosts and direct backend ports are inaccessible from the internet.

**Caddy cache milestone selected:** implement the pinned Souin-based
`caddyserver/cache-handler` in a controlled Caddy service image. Follow the
[Jellyfin cache plan](JELLYFIN-CADDY-CACHE.md): image caching first, then prove
video/range support against the former nginx behavior. Keep disk state bounded
and outside Syncthing/snapshots. No nginx cache fallback or automatic waiver is
selected. An unsupported video feature remains an explicit migration gap; do
not claim slicing parity or enable unsafe caching to close it.

### 6. Gokapi mixed exposure — implemented, desktop acceptance parked

Donbot declares the pinned Gokapi module. Its private admin UI uses verified
Tailscale identity and native header-auth accounts, with no second password.
Homer lists both administration and public sharing under Files; the public root
serves a static landing message. `send.<zone>` is a separate public sharing origin
on the existing DNS/token lifecycle. Version 8 Caddy credentials bind the reviewed public method/path
allowlist; unknown/encoded paths and administrative APIs remain denied.
Native download-password cookies and guest-upload keys are preserved; no caching.
The backend stays on one private Caddy network with no host listener.

Initial super-admin selection is explicit in
`skillet/environments/<prod|test>/hosts/donbot/gokapi/admin-user` (Password = one
lower-case allowed LoginName). Other allowlisted users get User rank with guest
upload capability; preserve existing roles, accounts, settings and native keys.
Native setup/account APIs own the database. Durable isolated staging recovers
interrupted first setup without rewriting existing live configuration.

Daily stopped-service tar backups include configuration, SQLite and file data;
retain seven completed sets in the caller's Syncthing backup scope. Temporary
files are excluded from synchronization until atomic publication. Restore saves
a recovery archive and validates/extracts the selected set before replacing live
state. No application data goes into KeePassXC. See
[Gokapi operation](../skillet/crates/gokapi/README.md).

**Next acceptance (deferred at operator request):** add the selected test admin
entry, update/reprovision the named Donbot VM with fresh Caddy credentials, and
use a desktop on the tailnet. Verify private admin/assets/API calls work without
an app password, correct native account/roles, revocation/unknown/tagged-user
denial and backend isolation. Exercise public share downloads, password cookies,
expiration/download limits and guest upload chunks without Tailscale. Verify
management remains denied with forged headers, unsupported methods and encoded
paths. Finish real systemd backup/restore, repeat-apply and reboot acceptance.
Provider-free native-consumer and real-proxy fixtures run in CI; they do not
close these manual/full-VM gates. Existing restored VM data remains untouched.
The passt VM still needs explicit LAN routing/forwarding for desktop public-route
checks; never substitute its mirrored workstation address or a Tailnet target.

### 7. Port required Joplin, Airtrail and PostgreSQL

Calculon's Chef and live Quadlets confirm one PostgreSQL container, database,
credential and data directory per application. Keep that isolation. Add a
reusable PostgreSQL Skillet module that Joplin and AirTrail callers can compose;
use named-container connectivity and Podman secret delivery, not pod-local
loopback or plaintext Quadlet environment values. The live Chef backup is a
daily `pg_dumpall` compressed with zstd and aged out after roughly seven days.
Replace that pattern with an atomic single-database logical archive and a
disposable restore path. Keep raw database files outside Syncthing; publish
completed archives atomically into caller-declared synchronized backup folders.
Coordinate AirTrail uploads and DB state as one restore set. Reuse shared SMTP
where needed without creating a remotely accessible unauthenticated relay.

Joplin's official server guide says item content is stored in PostgreSQL by
default; filesystem/S3 storage is optional. Keep the database-backed default.
The client-side JEX export is supplementary and does not restore server
accounts/metadata. If a non-database content store is selected later, back it up
with a consistent database snapshot. See
[PostgreSQL app backup design](../design/postgres-app-backups.md).

For Joplin, implement public trusted HTTPS through Caddy, retaining
Joplin's native account/session authentication for mobile and desktop sync. Set
`APP_BASE_URL` to the exact canonical HTTPS endpoint. Keep native authentication
enabled; do not replace it with browser OAuth redirects or an extra proxy login.
The canonical endpoint is `https://notes.<zone>`, using the environment's resolved
zone root rather than its private UI domain.
The public Joplin route must not require a tailnet identity or Tailscale connectivity.
Preserve API methods, headers and bodies. Validate background synchronization,
reconnect and note/attachment changes on the actual phone with Tailscale disconnected.
Retain native account isolation and application-owned sharing settings.

For Airtrail, prefer the same verified tailnet-user authentication with no
second application login, if its supported authentication/account model allows
it. Investigate trusted external identity support and account mapping before
implementation; a network restriction alone does not establish an application
session. If unsupported, retain native login behind the tailnet gate and record
the limitation rather than disabling authentication unsafely. Preserve required
application accounts/ownership. Inventory public API clients separately; do not
inherit the legacy blanket `/api/*` exemption.
The second synchronization/browser tenant stays deferred; preserve its source
state without importing it or selecting its snapshots. No multi-instance API
extension is required by this slice.

**Core service and public-ingress implementation complete; staging DNS-01 and
VM-local HTTPS passed. External reachability and Joplin client acceptance remain open.** The operator confirmed both current private service UIs work.
Joplin mobile/desktop authentication, bidirectional note/attachment sync and
reconnect must work over trusted public HTTPS without Tailscale; unauthorized
sync requests must be denied by Joplin. Verify AirTrail account and
attachment/API behavior. Joplin restore and AirTrail paired DB/upload restore
passed in disposable state. Repeat apply/reboot must preserve app/database
state. No live production peers receive restored smoke changes.

### 8. Fresh-install rehearsal and hardware deployment

Provision Donbot with new application state. Prove backups and restores against
synthetic disposable data; do not copy Calculon's application databases,
accounts, credentials or service state. Arr media is not part of provisioning;
the operator may later copy selected media and set ownership/ACLs separately.
Snapshot only caller-selected leaves; never snapshot all `/var/lib/data`. Raw
database clusters and temporary caches must not be synchronized.

**Exit:** all retained services have acceptance evidence; public HTTPS succeeds
from an external client without Tailscale; private/mixed denials are tested;
backup restore, reboot and repeat apply pass; DNS has one declared owner and
no DDNS updater is selected. An authorized client can route through donbot as an
exit node, including DNS and required IP families; this acceptance is separate
from ordinary disposable provisioning. Physical deployment is a fresh install;
do not present VM or image publication checks as physical acceptance.

## Validation and handoff

Each code slice runs the applicable checks from `AGENTS.md`, with routine CI
requiring no vault or external API. Real tests use a named disposable VM through
the Rust lifecycle owner; cache unlock prompts never block independent work.
Do not install workstation tools. Record honest evidence in
[ACCEPTANCE.md](archive/ACCEPTANCE.md), update designs and this plan, and maintain
the shared secret-requirement catalog/root checklist when exact entries are added.
The shared identity allowlist is implemented at `skillet/tailscale/ui-users`.
Gokapi uses explicit initial ownership and registered native accounts; its
desktop/Tailscale, public-flow and full VM acceptance are parked until desktop
access is available.

**Current milestone:** finish named-VM linked Arr acceptance (slice 4), including
harmless transfer/import with callbacks and authorized private browser checks.
The code, generated-key flow, real connection tests and application repeat/reboot
persistence have passed. Test credentials are present. The next implementation
milestone is external client sync and playback with a reachable ingress
destination and edge routing. Staging DNS/ACME and VM-local public-route checks
have passed for Joplin and Jellyfin; Plex is installed behind a claim/setup gate. Generic public
ingress, fixed names, migration and CI coverage are implemented. Separately complete AirTrail
account/API acceptance in the running named VM. Their database restore and
AirTrail paired upload restore have passed. Public internet acceptance remains required
before claiming production readiness for the media services.

## Existing shared file browsing

Mikochi and the generic LocalAPI identity gate are implemented for existing
Syncthing callers (clamps and joey), including encrypted allowlist delivery,
canonical DNS/Caddy declarations, ownership, private backend and CI regressions.
Donbot's profile includes this shared composition; its file-browsing acceptance
is still separate from the operator's confirmed Clamps browser checks.
Gokapi account mapping is implemented; desktop/Tailscale and Headscale acceptance
remain pending. See
[Butane acceptance](archive/ACCEPTANCE.md) for live checks and limitations.


## Resolver topology follow-up

Select and provision donbot's third Pi-hole explicitly, with private DNS policy
and independent state, to complement the colocated clamps/joey pair. Record
remote-client reachability and failure-domain behavior without inferring it from
UI DNS publication. See [fleet topology](../design/fleet-hosts.md).
