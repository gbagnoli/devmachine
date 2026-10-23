# Private Caddy UIs

Host callers declare services, container upstreams and optional aliases. Names
are `<service>.<host>.<ui-domain>`; Caddy publishes ports 80 and 443 over
IPv4 and IPv6. For each configured hostname, Caddy redirects HTTP to HTTPS and
adds an HSTS header with a one-year max age to HTTPS responses. HSTS is scoped
to configured hostnames and does not include subdomains. Run Cargo commands from
`ublue/skillet`.

The shared [Homer dashboard](../homer/README.md) uses the machine name
`<host>.<ui-domain>` and also has a zone-level `ui.<host>.<zone>` alias. Both use
the same private authorization gate as its canonical UI name. Skillet generates
its links from selected UI sites.

## Credentials and DNS

Complete [Cloudflare issuer/zone setup](../workstation/docs/cloudflare.md).
CLI environments are `production`/`test`; vault groups are `prod`/`test`.
Production delivery creates/reuses the vault child token at
`skillet/environments/prod/hosts/<host>/cloudflare/acme-token`. Never deliver the
master issuer token. Smoke UI provisioning creates a bounded disposable token.

Skillet publishes DNS-only machine A/AAAA records for its Tailscale addresses,
canonical UI CNAMEs to that machine name, and aliases to the canonical UI name.
Homer's machine-name alias uses the existing A/AAAA; it adds no CNAME there.
Do not enable Cloudflare proxying or configure Tailscale split DNS for these names.
Clients need tailnet connectivity and a resolver that accepts these DNS answers.

## Provision

```sh
cargo run --release -p skillet -- secrets deliver HOST caddy --target USER@ADDRESS --activate
# Existing owned disposable VM, with staging ACME:
cargo run --release -p skillet -- test vm provision HOST INSTANCE --with-ui
```

Tailscale must be enrolled first. [Remote apply](../workstation/README.md#apply-a-production-host)
handles that order. Caddy delivery installs `caddy_sites` and
`cloudflare_acme_token` before its consumer runs. Inspect `caddy.service` and
`skillet-caddy-apply.service`; the generated Caddyfile is under `/etc/skillet`.
Certificates persist with Caddy state under `/var/lib/data`.

Before changing the zone/prefix on a disposable run, dispose of it with its
recorded configuration. Destroy removes owned records and revokes child tokens;
keep the workstation issuer available for cleanup. Staging certificates are
untrusted; the staging test performs its own certificate checks.

## Identity-protected UIs

Pi-hole, Syncthing (canonical name and aliases), Mikochi and Home Assistant use
identity protection. Home Assistant also retains its native login.

For caller-declared identity protection:

1. Put complete allowed Tailscale `LoginName` values in Password at
   `skillet/tailscale/ui-users`, separated by commas or newlines; Username is
   unused. Obtain names with `tailscale status --json` on an authenticated client.
2. In Tailscale Access controls, grant those users access to server/smoke tags on
   `tcp:443`. Tag ownership alone does not grant connectivity.
3. Audit the vault, then redeliver Caddy credentials after changing the allowlist.

Requests from unknown, tagged or disallowed clients are denied. The authorization
socket is `/run/skillet-tailnet-ui/auth.sock`; tailscaled's socket is
`/run/skillet-tailscale/tailscaled.sock`. Do not forward private backend ports or
mount these sockets into unrelated containers. On SELinux hosts, inspect
`skillet-tailnet-ui-policy.service` as well as `skillet-tailnet-ui.service`;
the loader must succeed before the broker starts. The managed policy is
`/etc/skillet/selinux/skillet-tailnet-ui.cil`. Keep enforcement enabled. The backend needs Netavark
`isolate=strict` support. [Mikochi](../mikochi/README.md) uses this gate without
an additional application login. Headscale identity acceptance is not established.

Designs: [private access](../../../design/private-ui-access.md),
[identity gate](../../../design/tailnet-ui-authentication.md),
[Cloudflare lifecycle](../../../design/cloudflare-ui-lifecycle.md).
Acceptance backlog: [UI plan](../../../plan/GENERIC-PRIVATE-UIS.md).

## Host-network upstreams

Caller declarations select bridge DNS or `HostLoopback` transport. Host-loopback
services bind their HTTP listener locally and use a dedicated confined Unix relay
mounted only into Caddy. Version 4 delivery carries the transport; retained bridge
payloads keep their existing defaults. Redeliver Caddy credentials after adding a
new host UI. Native application authentication remains application-specific.
