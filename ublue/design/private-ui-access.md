# Private web UI access

Use one private hostname per administrative UI, Caddy HTTPS and named-container
bridge upstreams. Separate hostnames avoid subpath incompatibilities. Keep DNS
and Syncthing transfer listeners separate from web UI access; never publish an
unauthenticated backend as a recovery route.

## Connectivity and naming

Host-network Tailscale gives the server a normal routing interface. Caller policy
selects routing; production and smoke have separate tags/access rules. One-use
keys reach hosts while workstation OAuth remains private. [Routing](tailscale.md).

Own Cloudflare DNS-only names survive changing tailnet control planes and avoid
a Pi-hole/split-DNS dependency for UI resolution. Machine A/AAAA point to verified
tailnet addresses, UI CNAMEs to that machine and aliases to canonical UIs. Domains
and IDs stay in private configuration; public DNS still exposes names/addresses.
[Cloudflare lifecycle](cloudflare-ui-lifecycle.md) owns issuance and records.

The shared Homer dashboard follows the same private Caddy and identity path.
It reads links from the caller's selected UI services, so it has no independent
service inventory. Its zone-level alias is `ui.<host>.<zone>`; generated names
and the zone remain environment configuration. Skillet renders Homer YAML from a
checked-in template, and Caddy serves that YAML after authorization. The Homer
container supplies only packaged static assets and has no published port.

Matching IPv4/IPv6 wildcard port publications render as one native dual-stack
Podman mapping. Explicit single-family/address bindings retain their scope.
Overlapping wildcard mappings can leak Netavark nftables rules on restart;
rendering changes do not repair already-retained runtime rules.

Caddy DNS-01 supplies publicly trusted certificates without a public UI route.
DNS-only records do not use a Cloudflare proxy certificate. Resolver rebinding
filters can reject tailnet-address answers; actual client resolution is its own
acceptance scope. ACME propagation uses explicit public resolvers, independent
of the host/Pi-hole path; this does not prevent external issuer DNS failures.

## Authorization

Tailnet source restrictions and grants protect connectivity; they do not create
an application session. Retain existing application passwords unless that route
explicitly selects [verified tailnet identity](tailnet-ui-authentication.md).
File browsing and Gokapi administration require no second login; their identity
policy rejects unknown/tagged/disallowed sources and failed lookups. An address
range alone is not user authentication.

The identity and host-relay SELinux policy loaders share one transaction store.
Order the relay loader after the identity loader during concurrent boot, without
activating an otherwise absent optional service. Verify the real HTTPS listener
after reboot separately from local fixture TLS.

Private backends must not bypass Caddy. Syncthing's GUI port is unpublished;
identity-protected file browsing has its own strictly isolated bridge. Host root
remains trusted. Retain SSH/tailnet-device access for DNS recovery.

Public/mixed ingress is a distinct [proposed access contract](public-service-access.md),
not a relaxation of private defaults. Implementation and acceptance are tracked
in [private UIs](../plan/GENERIC-PRIVATE-UIS.md) and
[donbot migration](../plan/DONBOT-MIGRATION.md). Resolver deployment/site redundancy
belongs to the host migration plans, independent of UI DNS publication.

Instructions: [Caddy](../skillet/crates/caddy/README.md).
References: [DNS-only records](https://developers.cloudflare.com/dns/proxy-status/),
[rebinding](https://tailscale.com/docs/reference/faq/dns-rebinding/),
[DNS-01](https://letsencrypt.org/docs/challenge-types/),
[Caddy DNS challenge](https://caddyserver.com/docs/caddyfile/directives/tls).
