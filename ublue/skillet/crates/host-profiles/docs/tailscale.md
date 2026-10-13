# Tailscale setup

The caller selects a rootful host-network Tailscale container. State is under
`/var/lib/data/tailscale`. Run Cargo commands from `ublue/skillet`.

## OAuth and access controls

1. Define `tag:skillet-provisioner`, `tag:skillet-server` and `tag:skillet-smoke`
   in Tailscale Access controls. Set `tagOwners` so the provisioner owns the
   server/smoke tags. Give smoke devices only the intended test access.
2. In Trust credentials, create an OAuth client restricted to the provisioner
   tag with Auth Keys Write (`auth_keys`) and Devices Core Read/Write
   (`devices:core`). DNS-management scopes are unnecessary.
3. Store Client ID in Password at `skillet/tailscale/provisioner-client-id` and
   Client secret in Password at `skillet/tailscale/provisioner-client-secret`.
   Keep the OAuth client on the workstation; only a one-use key reaches the host.
4. For private UIs, configure [Cloudflare DNS](../../workstation/docs/cloudflare.md)
   and grant intended users `tcp:443` access. No MagicDNS/split-DNS setup is needed.

## Provision

```sh
cargo run --release -p skillet -- secrets unlock
cargo run --release -p skillet -- apply --host HOST --target USER@ADDRESS
# Existing owned disposable VM:
cargo run --release -p skillet -- test vm provision HOST INSTANCE
```

Remote apply stages Pi-hole and Tailscale before full apply and reuses enrollment
on repeats. For deliberate production re-enrollment, use
`secrets deliver HOST tailscale --target USER@ADDRESS --activate`.
That consumer is full apply, so other selected applications may also configure.
Retained raw-key credentials default to no exit-node advertisement; redeliver to
apply the caller-selected production preference.

Smoke enrollment is one-use, preauthorized, non-ephemeral and expires after one
hour if unused. Repeat provision preserves its identity. Add `--with-ui` for
Caddy staging. Destroy through the [VM owner](../../vm/README.md); it removes
Cloudflare resources and the matching smoke-tagged device before local artifacts.
Keep OAuth and Cloudflare issuer credentials available until cleanup finishes.

## Exit node and DNS

When the caller enables exit-node advertisement, approve the production device
in Tailscale's admin console and select it on clients. Restrictive access policy
must allow intended users `autogroup:internet`. Smoke provisioning disables
advertisement. Inspect `tailscale.service` and the container's status/preferences.
`TS_ACCEPT_DNS=false` preserves host resolver configuration.

Hosted enrollment/cleanup uses the Tailscale provider API. Do not assume Headscale
is a drop-in accepted backend. Existing LAN subnet routes are a separate caller
capability; exit-node advertisement does not configure them.

Design: [routing policy](../../../../design/tailscale.md).
