# Private Caddy UIs

Host callers declare services, container upstreams and optional aliases. Names
are `<service>.<host>.<ui-domain>`; Caddy serves HTTPS on port 443. Run Cargo
commands from `ublue/skillet`.

## Credentials and DNS

Complete [Cloudflare issuer/zone setup](../workstation/docs/cloudflare.md).
CLI environments are `production`/`test`; vault groups are `prod`/`test`.
Production delivery creates/reuses the vault child token at
`skillet/environments/prod/hosts/<host>/cloudflare/acme-token`. Never deliver the
master issuer token. Smoke UI provisioning creates a bounded disposable token.

Skillet publishes DNS-only machine A/AAAA records for its Tailscale addresses,
canonical UI CNAMEs to that machine name, and aliases to the canonical UI name.
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

Syncthing (canonical name and aliases) and Mikochi use identity protection.

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
mount these sockets into unrelated containers. The backend needs Netavark
`isolate=strict` support. [Mikochi](../mikochi/README.md) uses this gate without
an additional application login. Headscale identity acceptance is not established.

Designs: [private access](../../../design/private-ui-access.md),
[identity gate](../../../design/tailnet-ui-authentication.md),
[Cloudflare lifecycle](../../../design/cloudflare-ui-lifecycle.md).
Acceptance backlog: [UI plan](../../../plan/GENERIC-PRIVATE-UIS.md).
