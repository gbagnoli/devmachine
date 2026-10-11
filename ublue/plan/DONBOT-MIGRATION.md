# Calculon → donbot migration

Status: **draft, inventory first**, 2026-10-11. Source inventory is complete;
private/live inventory and donbot implementation remain pending. The shared
file browser and identity gate are implemented for clamps/joey; their live
acceptance is tracked separately below. Read
[DONBOT-INVENTORY.md](DONBOT-INVENTORY.md) before changing code. Follow the
[refactoring roadmap](SKILLET-REFACTOR.md) and keep its parked acceptance visible;
this plan does not close those gates or claim fresh donbot acceptance.

## Scope and prerequisites

Retain Arr/putioarr, Plex, Jellyfin, Gokapi, Syncthing and a private file-browsing
UI. Joplin and Airtrail are retain candidates; establish their effective
mode/use in slice 0. **Defer the magiusstaff tenant**, including its second
Syncthing/Filebrowser instance and snapshot selection. Do not add multi-instance
support for this migration. Preserve its source data. Exclude AdventureLog,
Wanderer and Openclaw.

Reuse the existing fleet baseline. Do not port Chef host/Podman setup. Only add
OS image packages where an actual service requires them; applications stay in
rootful containers. `ucore-images/recipes/ucore-donbot.yml` exists as a thin image;
a donbot profile/binary/Butane configuration does not exist yet. Do not clone
clamps-specific service choices such as UniFi or Home Assistant.

Accepted access requirements: former OAuth-protected UIs become tailnet-only;
Plex/Jellyfin are reachable publicly without a tailnet client; mixed services
keep explicitly required public flows. HTTPS is mandatory for all web endpoints.
Architecture constraints and proposals are in
[public service access](../design/public-service-access.md).

## Implementation slices and exit criteria

Implement one slice at a time, with focused validated commits. Do not silently
turn the source inventory into an assumption about running state.

### 0. Complete the effective inventory

Read effective private Chef settings and live service/container/nginx definitions
without recording credentials. Record privately: image versions/digests,
application URLs and accounts, public IPv4/IPv6 and NAT/firewalls,
media sizes/mounts/subvolumes, hardware acceleration, backup/restore state, and
actual Jellyfin cache hits/load. Check manual putioarr binary overrides and the
Plex/Syncthing firewall resource collision.

**Exit:** each retained service has a version, state/mount map and access policy;
resolve Joplin public versus private mode, Airtrail public API
clients, conditional Pi-hole synchronization, source subvolumes inside
synchronized data, and public reachability.
Keep private evidence outside Git. No source service is stopped or altered.

### 1. Add donbot using existing baseline

Add the canonical host profile, thin host binary, Butane selector/install inputs
and matching image policy. Select existing Syncthing, host-network Tailscale,
btrbk, Datadog and SMTP. Include Pi-hole for the intended third resolver and
mask resolved in the host image if selected. Choose a data account mapping that
preserves the source's shared media permissions without assuming workstation IDs.
Keep `/dev/vda` VM inputs and physical disk/root-profile choices distinct.

**Exit:** profile drives credentials/UIs/monitoring/readiness; strict Butane and
local CI pass; a fresh named donbot VM reaches signed-image base readiness,
repeat apply and reboot preserve the shared data mount. No source peer identity,
production routes or public DNS writer is imported into a smoke VM.
Shared subnet routing/private DNS gaps stay tracked in joey's plan. If live
Nebula Sync is needed, add it as an explicit shared capability with private
primary/replica delivery and a single production owner; smoke must not replicate
production settings or leases.

### 2. Extend generic ingress and verified tailnet authentication

The current Caddy renderer makes **every** declared UI tailnet-only and the
Cloudflare lifecycle targets tailnet machine addresses. Extend the shared typed
model with explicit public/private policy and mixed path/method rules; default
to private. Host callers select services and policy; private values come from
templates/vault. Keep one renderer/delivery/provider ownership path, with a
versioned migration for retained private payloads.

Add a reusable caller-selected identity gate for file browsing and Gokapi admin,
following [tailnet UI authentication](../design/tailnet-ui-authentication.md).
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
different owners: select DDNS only when needed and reject competing writers.
Public sites use DNS-only records by default. Advertise AAAA only when routed
IPv6 has been verified. Public 443 must work with Tailscale disconnected; port
80 may redirect to HTTPS, but never serves an application in plaintext.

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

### 3. Add shared private file browsing

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
matcher alone does not satisfy this requirement. The shared authentication work
in slice 2 is a dependency before activating this UI. Confine the unauthenticated
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

### 4. Port Arr and download integration

Use a cohesive media module with typed per-app configuration and shared helpers,
not a new broad system trait or a parallel host service registry. Declare
Prowlarr/Radarr/Sonarr/Lidarr and putioarr as separate bridge containers. Replace
pod loopback references with container DNS; prefer dedicated private hostnames
and deliberately migrate old application URL bases.

Resolve Put.io credentials, putioarr password and Arr API keys through existing
templates/delivery. Avoid literal secret environments; use supported secret
files or existing secret-aware runtime configuration. Preserve restored API keys
unless an intentional rotation updates every consumer. Review latest/nightly
channels and database compatibility before enabling automatic updates.

Declare media/download/library mounts and identity mappings once. Prefer common
container paths for downloads/imports, following the
[Servarr mount guidance](https://wiki.servarr.com/en/sonarr/installation/docker);
verify hardlinks/atomic rename on the
actual filesystem instead of assuming every Btrfs subvolume shares that behavior.
Preserve separate synchronized music policy explicitly. Replace the age-only
cleanup job with a confirmed completed-download policy or leave it disabled.

**Exit:** restored synthetic app configs can connect over bridge DNS, a harmless
sample transfer/import reaches the right library with correct ownership, private
UI routes work and no backend UI port is published. Repeat apply/reboot preserve
app state; secrets are absent from Quadlets/recordings. Any path remapping and
copy-versus-hardlink behavior is documented.

### 5. Port Jellyfin, then Plex, with public HTTPS

Use separate public hostnames, Caddy TLS and named bridge upstreams. Keep native
application authentication; backend HTTP remains confined to the bridge.
Migrate Jellyfin's known-proxy/client-IP policy, library paths and WebSockets.
Inspect the legacy `/dara/tvshows` path before fixing it. Declare GPU access only
for hardware actually present; preserve a CPU-only configuration.

For Plex preserve server identity and library state. Publish the explicit custom
access URL `https://<plex-hostname>:443`, and test intended Plex clients through
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

### 6. Port Gokapi's mixed exposure

Pin a compatible version and restore config plus file/metadata state together.
Replace OAuth header identity with the shared verified tailnet-user gate.
Use Gokapi header authentication with the complete verified identity mapped to
precreated application accounts for the two allowed users; no internal password
prompt. Decide their application roles explicitly and disable automatic creation
of arbitrary users. Public sharing routes get no administrative identity. Derive a version-specific public
path/method allowlist for downloads, guest uploads/chunks and required assets;
keep admin, account, API-key and management routes private. Do not translate the
broad legacy `^/d.*` exemption directly. Strip untrusted identity headers.

Preserve the public sharing URL and existing links where required. Decide one
mixed hostname versus separate administration hostname based on supported base
URLs/cookies/origin behavior. Prefer a tailnet-addressed admin hostname and
public-addressed sharing hostname; verify that generated links and frontend API
calls use the correct entry point. Being connected to Tailscale does not identify
a browser request sent over the public IP. Carry file/chunk size and timeout requirements;
`GOKAPI_USE_CLOUDFLARE` follows actual proxy topology, not DNS provider choice.

**Exit:** without Tailscale, an existing/restored share downloads and a guest
upload completes; expiration/limits remain effective. Admin routes and APIs deny
external clients even with forged headers, encoded paths or alternate methods.
Tailnet admin authentication works. Backup/restore retains metadata/files and
repeat apply preserves application-managed configuration. Do not cache share
responses unless expiry and counters are proven unaffected.

### 7. Port PostgreSQL-backed apps

Add a reusable PostgreSQL composition/backup helper for Joplin and Airtrail;
use named-container connectivity and secret delivery, not pod-local loopback.
Keep each app's state/version/credential declaration authoritative. Restore
logical backups into a compatible database major before planning upgrades.
Publish completed backup archives atomically into declared synchronized backup
folders; coordinate Airtrail files and DB state. Reuse shared SMTP where needed
without creating a remotely accessible unauthenticated relay.

Implement Joplin's chosen public/private mode and Airtrail's agreed API policy.
The second synchronization/browser tenant stays deferred; preserve its source
state without importing it or selecting its snapshots. No multi-instance API
extension is required by this slice.

**Exit:** Joplin client sync and Airtrail attachments/API work in their selected
modes, backup restore succeeds on disposable state, and repeat apply/reboot
preserve app/database state. No live production peers receive restored smoke
changes.

### 8. Production rehearsal and cutover

Record per-service copy/restore/rollback steps and verified backups, capacity,
ownership/ACLs and final path mappings. Snapshot only caller-selected leaves;
never snapshot all `/var/lib/data`. Raw database clusters and temporary caches
must not be synchronized. Rehearse using isolated state before final copy.

**Exit:** all retained services have acceptance evidence; public HTTPS succeeds
from an external client without Tailscale; private/mixed denials are tested;
backup restore, reboot and repeat apply pass; DNS/DDNS has one production owner.
Stop source writers for the final consistent copy and preserve calculon for
rollback. Excluded app data is left untouched. Do not describe image publication
or VM tests as physical deployment or production acceptance.

## Validation and handoff

Each code slice runs the applicable checks from `AGENTS.md`, with routine CI
requiring no vault or external API. Real tests use a named disposable VM through
the Rust lifecycle owner; cache unlock prompts never block independent work.
Do not install workstation tools. Record honest evidence in
[ACCEPTANCE.md](../butane/ACCEPTANCE.md), update designs and this plan, and maintain
the shared secret-requirement catalog/root checklist when exact entries are added.
The shared identity allowlist is implemented at `skillet/tailscale/ui-users`.
Gokapi application-account mapping and other migration-specific inputs remain
to be designed.

**Next action:** slice 0, a bounded read-only effective inventory. First code
slice is donbot baseline/profile, reusing the shared tailnet authentication
and file UI after their acceptance gates. Public ingress must be validated before importing public
service state.

## Shared file browsing implementation (2026-10-11)

Mikochi and the generic LocalAPI identity gate are implemented for existing
Syncthing callers (clamps and joey), including encrypted allowlist delivery,
canonical DNS/Caddy declarations, ownership, private backend and CI regressions.
Donbot still needs its profile; the shared composition will include file browsing.
Gokapi identity/account mapping and Headscale acceptance remain pending. See
[Butane acceptance](../butane/ACCEPTANCE.md) for live checks and limitations.
