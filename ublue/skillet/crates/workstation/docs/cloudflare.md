# Cloudflare workstation credentials

Create an **account-owned API token** with:

- **Account → API Tokens → Write** (Read can also support permission discovery).
- **Zone → Zone → Read**, restricted to the selected production/test zones.

Save it in Password at `skillet/cloudflare/token-creator`. Keep the issuer on
the workstation. Do not deliver it to a host or grant it DNS Write for this flow.

In each selected environment, create Password entries:

| Entry | Value |
| --- | --- |
| `skillet/environments/<environment>/dns/cloudflare-zone-id` | Zone ID copied from that zone's Overview page |
| `skillet/environments/<environment>/dns/ui-domain` | Optional relative prefix; absent defaults to `ui` |

`<environment>` is `prod` or `test`. Skillet fetches the zone domain and account
ID from the mandatory Zone ID. For zone `example.com`, `ui.whatever` resolves to
`ui.whatever.example.com`. An empty prefix is invalid; a full domain is also
appended to the zone, so enter only a relative prefix.

Delivery creates zone-scoped child tokens with Zone Read and DNS Write through
account-token endpoints. Production child tokens are saved in the vault;
disposable tokens and records are tracked by the VM lifecycle. Do not create
manual smoke ACME tokens. Keep the issuer available until owned cleanup finishes.

Before changing a disposable deployment's zone/prefix, destroy it with its
recorded configuration. Cleanup rejects a different resolved namespace.

Use [Caddy delivery](../../caddy/README.md) for UI certificates and records, or
[DDNS delivery](../../ddns/README.md) for a public-address updater.

Design: [Cloudflare ownership lifecycle](../../../../design/cloudflare-ui-lifecycle.md).
