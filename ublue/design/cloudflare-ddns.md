# Public address DDNS

Decision, 2026-10-06: retain Rupik's public-address DDNS capability. Plan to
reuse the existing Chef image family in a rootful Quadlet container, with
caller opt-in and private configuration supplied through Skillet credentials.
This preserves the existing updater instead of introducing another guest
Cloudflare API client. Guest convergence, production credential delivery, and
journaled disposable token/record cleanup are implemented. Live disposable DNS
acceptance remains pending the test-only DDNS record-name entry.

The host caller explicitly opts into the shared DDNS crate/service. Installing
Skillet or configuring an environment zone does not enable DDNS. Reuse
`skillet/environments/<environment>/dns/cloudflare-zone-id`; no separate DDNS
zone entry is needed. The public JSON policy is an embedded workstation
configuration template. KeePassXC contains only each environment/host's
relative DDNS record name; see the [configuration template design](configuration-templates.md).

The DDNS container owns only its explicitly configured public-address records.
The existing workstation UI lifecycle owns tailnet machine records and UI
CNAMEs. Keep those record sets disjoint. One production writer owns the retained
DDNS records during cutover; stopping Rupik's writer precedes starting Clamps'.

Match Chef's initial policy: A enabled, AAAA disabled, TTL 300, unknown-record
purging disabled. Preserve each record's approved proxy status. Do not derive
DDNS names from the UI namespace or enable IPv6 without a usable public address
and an explicit decision.

The workstation issuer supplies a separate zone-scoped DDNS child token;
the token creator stays on the workstation. Deliver the complete config as an
encrypted credential and mount its Podman secret at the updater's config path.
Avoid a plaintext token-bearing config file on persistent host storage. Podman's
own disk secret copy remains covered by the planned storage encryption policy.
Use existing atomic delivery, deferred activation, and lifecycle ownership.

The selected image is upstream 2.2.0, pinned to its multi-platform index digest.
Its scratch image runs as UID/GID 0 with `/` as its working directory and
`/cloudflare-ddns --repeat` as entrypoint. The legacy JSON loader reads
`/config.json`; its repeat interval is the configured TTL (300 seconds).
Mount the complete Podman secret there as `0400`, UID/GID 0. Do not set
environment-mode token variables, which would bypass JSON configuration.
An offline run of the actual image accepted this secret mount with dummy
credentials and networking disabled. Digest pinning intentionally prevents
registry auto-update from changing this evaluated contract.
Its legacy API path reads one page of 100 A records; provisioning refuses a
zone that would exceed that limit after adding the declared records.

The version 1 DDNS JSON template has relative `records` and explicit `proxied`
booleans. Its `$secret` marker resolves the `ddns-dns-name` vault leaf, and
`takeover_existing` defaults to false. A per-host template replaces the shared
template completely. Production preflight rejects
the whole UI namespace, duplicates, and incompatible records. Existing public
A records need explicit takeover approval; records owned by other Skillet
consumers cannot be adopted. The updater writes a distinct
`skillet-ddns:production:<host>` comment so later delivery recognizes its
records. Approval does not stop an old writer; that remains a cutover action.

Production delivery reuses or creates a separate persistent child token through
the same atomic token-persistence helper used by Caddy. New tokens are revoked
when vault saving fails; deterministic token names support retry recovery.
The dedicated DDNS apply phase leaves ordinary full apply independent of this
credential. Test-policy delivery is opt-in through the disposable VM provision
flow. It journals exact test record names, ownership markers, record IDs, and
bounded token identities before mutation; cleanup validates this state against
the recorded VM and deletes only records that still match. Ordinary application
smoke checks exclude DDNS.

Source contract: [upstream 2.2.0](https://github.com/timothymiller/cloudflare-ddns/tree/7c6d5b43c1d400f4977f2b25bd44d784bb601ae3).

Disposable tests use explicitly isolated record names and bounded credentials.
Journal and clean only test-owned resources; disposal never removes retained
production DDNS records. The first VM attempt was parked before Cloudflare
mutation because its then-required private JSON config entry was missing. Live
acceptance now requires the test `ddns-dns-name` leaf. See the
[implementation plan](../plan/CLAMPS-CLOUDFLARE-DDNS.md) and [token/DNS
lifecycle](cloudflare-ui-lifecycle.md).
