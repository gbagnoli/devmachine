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
private payloads remain private until public redelivery. Live public DNS/ACME
and internet acceptance still need an explicit routable destination. See the
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
Tailscale. Domain/address values stay in private configuration.

Use Caddy for public TLS and bridge proxying with native application authentication.
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

Current code supports explicit public and private sites, with private defaults
and caller-bound version 6 public payloads. Matching retained older payloads keep
their private canonical name and identity gate until redelivery. Public Joplin
retains native authentication and its unpublished bridge backend. Mixed
path/method policy is still unimplemented. Sources and execution
gates: [inventory](../plan/DONBOT-INVENTORY.md),
[migration plan](../plan/DONBOT-MIGRATION.md), and
[private UI access](private-ui-access.md).
