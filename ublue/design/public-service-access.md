# Public and mixed service access

## Requirements agreed 2026-10-11

Calculon's replacement, donbot, needs publicly reachable HTTPS for Plex and
Jellyfin without a Tailscale client. Former OAuth-protected administrative UIs
move to tailnet-only access. Gokapi retains public sharing/upload flows with
private administration; Airtrail's public API exception and Joplin's conditional endpoint require explicit
source policy rather than inheriting generic private access.

The private file-management capability should be shared by Syncthing hosts.
The magiusstaff second instance is deferred. File browsing requires no internal
password prompt; use the [shared tailnet identity policy](tailnet-ui-authentication.md)
for the two intended users and reuse it for Gokapi administration.

## Proposed access contract

Extend the existing generic Caddy and provider lifecycle with explicit service
access policies. Keep private as the default and deny unspecified mixed routes.
Public and tailnet machine records must have independent address destinations;
UI aliases inherit their service's policy. Reuse Cloudflare DNS-01 with scoped
credentials; public endpoints use DNS-only public addresses and do not depend on
Tailscale. Domain/address values stay in private configuration.

Use Caddy for public TLS and bridge proxying with native media authentication.
Decision, 2026-10-11: use Caddy caching for Jellyfin, via a pinned
`caddyserver/cache-handler` service image and tested filesystem storage. Start
with images, then prove protected video/range behavior; nginx slicing parity is
unverified. Cache hits must preserve authorization/revocation, and cache data
stays outside synchronized/snapshotted folders. The
[cache plan](../plan/JELLYFIN-CADDY-CACHE.md) owns implementation gates. See
[Jellyfin's proxy requirements](https://jellyfin.org/docs/general/post-install/networking/reverse-proxy/)
and [Caddy cache module status](https://caddyserver.com/docs/modules/http.handlers.cache).

The original [File Browser](https://github.com/filebrowser/filebrowser) is
archived. [File browsing](file-browsing.md) records the shortlist, with Mikochi
as the selected shared implementation on clamps and joey. Keep any
persistent app state outside synchronized user files and serve declared folders.

## Boundary

Current code serves every declared Caddy UI with a tailnet restriction; it has
no generic public or mixed policy. Existing private behavior remains authoritative
until the versioned extension is implemented and tested. Sources and execution
gates: [inventory](../plan/DONBOT-INVENTORY.md),
[migration plan](../plan/DONBOT-MIGRATION.md), and
[private UI access](private-ui-access.md).
