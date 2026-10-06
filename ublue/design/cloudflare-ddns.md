# Public address DDNS

Decision, 2026-10-06: retain Rupik's public-address DDNS capability. Plan to
reuse the existing Chef image family in a rootful Quadlet container, with
caller opt-in and private configuration supplied through Skillet credentials.
This preserves the existing updater instead of introducing another guest
Cloudflare API client. Implementation is pending; verify the chosen image's
config schema and Podman secret-file consumption before committing to its
runtime integration.

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

Disposable tests use explicitly isolated record names and bounded credentials.
Journal and clean only test-owned resources; disposal never removes retained
production DDNS records. See the [implementation plan](../plan/CLAMPS-CLOUDFLARE-DDNS.md)
and [existing token/DNS lifecycle](cloudflare-ui-lifecycle.md).
