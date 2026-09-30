# Private web UI access

## Decision

Use a private hostname per administrative UI, with Caddy terminating HTTPS and
proxying to the named container on the shared Podman bridge. Pi-hole and
Syncthing are the first targets. Their UIs are reachable only through the
tailnet; neither needs a public listener or an OAuth2 proxy. Keep Pi-hole's LAN
DNS listener and Syncthing's peer-transfer ports separate from their UIs.

Run Tailscale in its Podman container with `Network=host`, matching the
existing Chef configuration on rupik, calculon, and boxy. This gives the host
a normal Tailscale interface for host-level routing and port forwarding.
Persist its state under `/var/lib/data/tailscale`. Use separate tagged
identities and access rules for production servers and smoke VMs. The
workstation uses a narrowly scoped OAuth client to mint one-use enrollment
keys; only those short-lived keys are delivered to a host, never the OAuth
client secret.

Use host-specific names in existing Cloudflare-managed production and smoke-test
zones rather than `*.ts.net`, so UI URLs and Caddy routes can survive a move
between hosted Tailscale and Headscale. Publish DNS-only A/AAAA records pointing
to each host's tailnet address. Do not enable Cloudflare proxying. Keep literal
zones and records outside this public repository; public DNS still exposes the
names and addresses. Some resolvers block public answers containing tailnet
addresses as DNS rebinding, so check resolution from real clients before
cutover. Caddy gets publicly trusted certificates through Cloudflare DNS-01;
the UIs need no public route. Cloudflare's certificate is not used for DNS-only
records: Caddy serves the certificate.

Do not configure Tailscale split DNS for the UI names. A VM can enroll before
its DNS record exists. After enrollment, the workstation reads its tailnet
address and creates a DNS-only record with a short-lived token scoped to the
smoke-test zone. VM destruction deletes only its recorded DNS records and
revokes that token. Production records use a separately scoped credential;
update them if a host's tailnet address changes. Tailscale enrollment needs no
DNS-management OAuth scope. Keep the Cloudflare token creator on the workstation.

Plan a second Pi-hole on beelzebot beside clamps. They cover individual host
failure but share power and internet failure. A third Pi-hole on bender,
the calculon replacement in the other country, would add a separate site.
These Pi-hole instances provide redundant client DNS service, independent of
UI name publication in Cloudflare. A healthy resolver can resolve an offline
host's name but cannot make its UI available. Give each Pi-hole UI its own
host-specific name; do not promise a floating service name without health-aware
routing.

Restrict Caddy's UI listener to tailnet traffic and grant access only to the
intended user identities. Tailnet access authorizes the connection; it does not
create an application login. Keep application passwords until the private path
and absence of direct UI access have been verified, then decide per application
whether a second login is useful. In particular, Syncthing's current wildcard
host publication of GUI port 8384 must be removed or limited to a safe local
proxy path before relying on tailnet access alone.

Retain access to each host by its tailnet device name or address, and SSH
access for DNS recovery. Do not expose an unauthenticated UI as the recovery
path.

## Why

Separate hostnames avoid applications' subpath problems. Tailnet identity and
grants keep admin UIs private without maintaining a public OAuth2 proxy.
Cloudflare DNS-only records remove the Pi-hole and tailnet DNS configuration
dependency from UI name resolution, while keeping our own names across control
planes. The names and tailnet IPs are public DNS data.

## Status and references

Skillet now configures the host-network Tailscale container and manages tagged
enrollment for clamps smoke VMs, including device removal during VM disposal.
This has passed workspace checks but still needs live tailnet acceptance. Caddy,
Cloudflare UI records, and UI isolation have not been implemented or accepted
on clamps. The current Pi-hole UI is private to the container bridge;
Syncthing's GUI is still published on the host.

References: [Tailscale DNS](https://tailscale.com/docs/reference/dns-in-tailscale/),
[Cloudflare DNS-only records](https://developers.cloudflare.com/dns/proxy-status/),
[DNS rebinding](https://tailscale.com/docs/reference/faq/dns-rebinding/),
[Let's Encrypt DNS-01](https://letsencrypt.org/docs/challenge-types/),
[Caddy DNS challenge](https://caddyserver.com/docs/caddyfile/directives/tls).
