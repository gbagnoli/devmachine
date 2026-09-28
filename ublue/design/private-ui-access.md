# Private web UI access

## Decision

Use a private hostname per administrative UI, with Caddy terminating HTTPS and
proxying to the named container on the shared Podman bridge. Pi-hole and
Syncthing are the first targets. Their UIs are reachable only through the
tailnet; neither needs a public listener or an OAuth2 proxy. Keep Pi-hole's LAN
DNS listener and Syncthing's peer-transfer ports separate from their UIs.

Use names under a domain we control, rather than `*.ts.net`, so the UI URLs and
Caddy routes survive a move between hosted Tailscale and Headscale. Caddy gets
publicly trusted certificates through Cloudflare DNS-01 using the existing
secret-delivery design. DNS-01 does not require publicly reachable UI endpoints.

Hosted Tailscale MagicDNS cannot store arbitrary records. Configure split DNS
for the private UI zone, forwarding to the fleet's Pi-hole resolvers. Each
resolver must return the same host-specific UI records, pointing to the
relevant host's tailnet address. Headscale can instead supply the same names
through its `dns.extra_records`. Moving control planes requires updating the
tailnet addresses and DNS configuration, but not application URLs or Caddy
routes.

Plan a second Pi-hole on beelzebot beside clamps. They cover individual host
failure but share power and internet failure. A third Pi-hole on bender,
the calculon replacement in the other country, would add a separate site.
Advertise multiple resolvers for the hosted Tailscale split DNS zone only after
their records and reachability agree; clients may query them in any order, so
none is a fixed primary. A healthy resolver can resolve an offline host's name
but cannot make its UI available. Give each Pi-hole UI its own host-specific
name; do not promise a floating service name without health-aware routing.

Restrict Caddy's UI listener to tailnet traffic and grant access only to the
intended user identities. Tailnet access authorizes the connection; it does not
create an application login. Keep application passwords until the private path
and absence of direct UI access have been verified, then decide per application
whether a second login is useful. In particular, Syncthing's current wildcard
host publication of GUI port 8384 must be removed or limited to a safe local
proxy path before relying on tailnet access alone.

Pi-hole is a dependency for resolving private UI names under hosted Tailscale;
multiple independent resolvers reduce that dependency. Retain access to each
host by its tailnet device name or address, and SSH access for DNS recovery;
these must not depend on Pi-hole. Do not expose an unauthenticated UI as the
recovery path.

## Why

Separate hostnames avoid applications' subpath problems. Tailnet identity and
grants keep admin UIs private without maintaining a public OAuth2 proxy. Caddy
and our own domain names work with either control plane: Tailscale Services
currently requires Tailscale's hosted control plane, while Headscale offers
extra DNS records but not Tailscale Services.

## Status and references

This is the planned access design; Caddy, tailnet DNS, resolver redundancy, and
UI isolation have not yet been implemented or accepted on clamps. The current
Pi-hole UI is private to the container bridge; Syncthing's GUI is still
published on the host.

References: [Tailscale DNS](https://tailscale.com/docs/reference/dns-in-tailscale/),
[Headscale DNS](https://headscale.net/stable/ref/dns/),
[Headscale Services feature gap](https://github.com/juanfont/headscale/issues/2845),
[Let's Encrypt DNS-01](https://letsencrypt.org/docs/challenge-types/),
[Caddy DNS challenge](https://caddyserver.com/docs/caddyfile/directives/tls).
