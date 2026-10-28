# Public and mixed service access

## Requirements agreed 2026-10-11

Calculon's replacement, donbot, needs publicly reachable HTTPS for Joplin, Plex
and Jellyfin without a Tailscale client (Joplin decision updated 2026-10-27).
Former OAuth-protected administrative UIs
move to tailnet-only access. Gokapi retains public sharing/upload flows with
private administration. Airtrail's public API exception requires explicit source
policy. Joplin and Airtrail are required migration services.

The private file-management capability should be shared by Syncthing hosts.
The magiusstaff second instance is deferred. File browsing requires no internal
password prompt; use the [shared tailnet identity policy](tailnet-ui-authentication.md)
for the two intended users and reuse it for Gokapi administration.

## Application authentication policy

Arr browser UIs use the shared verified tailnet-user allowlist with no second
application login, through supported external-authentication configuration.
Internal integration calls retain their API keys and private network boundaries.
Airtrail should use the same user authentication where its account/session model
supports it; compatibility and identity mapping need verification. Retain native
login behind the tailnet gate if unsupported, documenting the limitation. Do not
mistake tailnet reachability for application authentication or carry forward a
blanket public API exemption.

## Joplin access

Decision, 2026-10-27: replace the planned tailnet-only policy with public trusted
HTTPS. Mobile/desktop clients must sync without Tailscale. Joplin retains native
account/session authentication; Caddy adds neither a tailnet identity requirement
nor a browser proxy login to this service. Its database and direct container
port remain private. Native account and sharing settings remain application-owned;
authenticated sync responses are not cached.

Configure the canonical HTTPS `APP_BASE_URL` consistently with the public DNS
destination and proxy hostname. Prove client authentication and bidirectional
note/attachment sync from outside the tailnet, including background sync and
reconnect. Generic public ingress and Joplin's caller are implemented; retained
private payloads remain private until public redelivery. Internet acceptance
needs an explicit routable destination and edge routing;
staging DNS-01 and VM-local probes do not establish client reachability. See the
[official server guide](https://github.com/laurent22/joplin/blob/dev/packages/server/README.md)
for native sync accounts and reverse-proxy setup.

## Public DNS names

Resolve `<zone>` from the environment's existing Cloudflare zone ID. Public
service labels are fixed in caller declarations, with no additional vault
configuration. They are independent of the private UI prefix and host identity:

| Application | Public HTTPS names |
| --- | --- |
| Joplin | `notes.<zone>` |
| Plex | `plex.<zone>` |
| Jellyfin | `video.<zone>`, `media.<zone>` |

For a private UI namespace `ui.example.invalid`, the public Joplin name is
`notes.example.invalid`. Derive it from the resolved zone name, not by stripping
text from the private UI domain. Apply the same rule in test and production.
Both Jellyfin names receive DNS records, TLS coverage and equivalent Caddy
proxy/authentication policy. Joplin's canonical `APP_BASE_URL` uses its public
`notes` hostname; Plex advertises its public `plex` hostname. Each record belongs
to the selected deployment's existing DNS ownership/cleanup lifecycle; shared
labels must not silently replace another deployment's records.
IP targets use a separate `public.<host>.<zone>` machine record with service
CNAMEs, while an existing public hostname can be selected directly. No target
is inferred from private or tailnet addresses.

## Proposed access contract

Extend the existing generic Caddy and provider lifecycle with explicit service
access policies. Keep private as the default and deny unspecified mixed routes.
Public and tailnet machine records must have independent address destinations;
UI aliases inherit their service's policy. Reuse Cloudflare DNS-01 with scoped
credentials; public endpoints use DNS-only public addresses and do not depend on
Tailscale. Domain/address values stay in private configuration. Test provisioning
may use
an explicit private LAN/ULA destination for the same native-auth HTTPS routes;
this does not establish internet reachability. Production IP destinations remain
public-only. Test addresses must not be inferred from passt's mirrored host IP,
and no test route may reuse the private UI's Tailscale DNS target.

Use Caddy for public TLS and bridge proxying with native application authentication.
Decision, 2026-10-11: use Caddy caching for Jellyfin, via a pinned
`caddyserver/cache-handler` service image and tested filesystem storage. Start
with anonymous static web assets (implemented 2026-10-28), then prove protected
artwork/video behavior before enabling those routes; nginx slicing parity is
unverified. The initial Simplefs cache uses a hard-capped 128 MiB tmpfs and a
512 MiB container memory limit. It is intentionally disposable rather than
persistent: cache recreation cannot grow a disk indefinitely or touch certificates.
Request credentials/queries/ranges and upstream cookies/private/no-store/non-200
responses are excluded. Native users, libraries and known-proxy settings remain
Jellyfin-managed; media mounts use Arr's data identity and read-only final paths.
Plex uses the same library roots, its own persistent identity/configuration and a
separate transcode directory. Its official init sets the worker's data UID/GID;
claiming uses SSH-forwarded loopback 32400. No discovery ports or proxy-subnet
authentication exemption are enabled. Until its native claim marker exists,
Caddy serves 503 without proxying; reapply Caddy after claiming. The marker is
observed on the host and cannot be enabled by delivered UI credentials. Plex
advertises the explicit HTTPS port 443. Cache hits must preserve
authorization/revocation, and cache data
stays outside synchronized/snapshotted folders. The
[cache plan](../plan/JELLYFIN-CADDY-CACHE.md) owns implementation gates. See
[Jellyfin's proxy requirements](https://jellyfin.org/docs/general/post-install/networking/reverse-proxy/)
and [Caddy cache module status](https://caddyserver.com/docs/modules/http.handlers.cache).

The original [File Browser](https://github.com/filebrowser/filebrowser) is
archived. [File browsing](file-browsing.md) records the shortlist, with Mikochi
as the selected shared implementation on clamps and joey. Keep any
persistent app state outside synchronized user files and serve declared folders.

## Boundary

Current code supports explicit public and private sites, with private defaults
and caller-bound version 6 public payloads. Matching retained older payloads keep
their private canonical name and identity gate until redelivery. Cache-enabled callers
use version 7; this does not change their declared exposure. Public Joplin
retains native authentication and its unpublished bridge backend. Mixed
path/method policy is still unimplemented. Sources and execution
gates: [inventory](../plan/DONBOT-INVENTORY.md),
[migration plan](../plan/DONBOT-MIGRATION.md), and
[private UI access](private-ui-access.md).


## Gokapi sharing boundary

Gokapi uses separate origins: its private caller UI for administration and
`send.<zone>` for public links. Version 8 credentials bind the additional public
site and a pinned route policy to the same declared private backend. It gets no
shared ingress network or host port. Public requests carry no verified user;
private requests receive the existing WhoIs-authorized identity on every request.

Use native header authentication with registered users only. An explicitly
selected allowlisted LoginName owns the initial super-admin account; remaining
allowlisted identities get User rank and guest-upload capability. Native setup
and account APIs own schema, salts and users; preserve later role changes and
never import or edit SQLite directly. Revocation in the proxy allowlist remains
immediate even if the native account survives. Private browser APIs stay on the
private origin; generated share/guest-upload links use the public ServerUrl.

The public policy permits only reviewed methods and paths for the pinned image.
Keep native download password cookies and guest request keys, strip identity
headers, deny encoded paths and unspecified routes, and cache nothing. Version
or public-surface changes require real consumer and proxy regression checks.
Backups stop the writer and capture config, SQLite and file data together;
publish seven retained sets into the existing synchronized backup scope.

Completed Gokapi backup sets are ordinary tar files. Stage them using the
synchronizer's temporary-file contract rather than a hidden directory, which
could still replicate unfinished children. Publication requires no user ignore
changes. See [Syncthing temporary names](https://github.com/syncthing/syncthing/blob/main/lib/fs/tempname.go).
