# Pi-hole

The caller must declare Pi-hole. Run Cargo commands from `ublue/skillet`.

## Credentials and apply

Clamps and Joey use [Caddy identity protection](../caddy/README.md#identity-protected-uis).
Open the Pi-hole hostname from an allowed person's untagged Tailscale client;
there is no additional Pi-hole password. DNS remains available on host port 53.

```sh
cargo run --release -p skillet -- apply HOST
```

Apply updates the private backend and redelivers the matching Caddy policy.
Do not clear a password manually on an old shared network. Existing DNS settings,
lists and query data are preserved. Old web-password vault entries are unused for
these hosts and may be removed after migration; Skillet does not delete them.

A caller without identity protection retains native login. Only that mode uses
`skillet/hosts/<host>/pihole/web-password` in KeePassXC Password and
`secrets deliver HOST pihole`; fresh native credentials are generated and saved,
while missing entries for an existing host require restoring the original entry.
Disposable native credentials remain separate from production.

## Service and storage

Pi-hole publishes host IPv4/IPv6 port 53 over TCP and UDP. The UI is private on
dedicated strictly isolated bridge port 8088 through [Caddy](../caddy/README.md), not published on the host.
Persistent configuration/logs are under `/var/lib/data/pihole/{etc,log}`.
Inspect `pihole.service` and its journal. Preserve a working bootstrap resolver
while Pi-hole is down; follow [native networking](../network/README.md) for DNS.

## Private DNS records

Create `skillet/environments/<environment>/dns/local-prefix` in Password: the
relative site name below the Cloudflare zone selected by the existing
`dns/cloudflare-zone-id`. The zone name is resolved through the workstation's
Cloudflare token creator. The default DDNS template uses this same prefix for
its public A record; local device records are delivered only to Pi-hole.

Under `skillet/environments/<environment>/dns/entries/`, create one entry per
device. Username/Password are unused; custom fields are:

| Field | Value |
| --- | --- |
| `names` | Optional comma/newline-separated relative names; defaults to the entry title |
| `ipv4` | Optional single IPv4 address |
| `ipv6` | Optional single IPv6 address |

At least one address is required. For example, title `printer`, prefix `home`
and zone `example.invalid` produce `printer.home.example.invalid`. Multiple
names are hosts aliases, not CNAME records. Do not enter full zone-qualified
names. Conflicting names, malformed addresses and nested entry groups fail
validation. Records inside the tailnet UI namespace are rejected to preserve
its existing access path. No DHCP configuration is created.

All selected production Pi-hole hosts receive the same inventory. Host records
are generated from caller-selected static networking: `<host>` uses wired when
selected, otherwise Wi-Fi; `<host>-wifi` names the additional Wi-Fi address.
Do not duplicate these names manually. Generated records currently use IPv4;
manual entries accept IPv6 independently. Test delivery reads only test entries
and never reads physical network addresses.

`skillet apply HOST` stages and applies DNS after the container is configured.
To deliver only local records:

```sh
cargo run --release -p skillet -- secrets deliver HOST pihole-dns --activate
```

Use `test vm provision HOST INSTANCE --with-pihole-dns` for an owned VM. With
`--activate`, only the DNS apply unit is started. On the guest it stops Pi-hole
when changes are needed, updates only `dns.hosts` in its existing v6 TOML file,
and restarts it. Other settings/comments are preserved. Activation failures
remain retryable. Routine container convergence no longer writes `custom.list`.
Existing legacy files are left intact for inspection; migrate their records
into the vault explicitly. Skillet owns the complete `dns.hosts` list: GUI edits
to that list are replaced on the next DNS apply. An empty inventory clears the
managed list; it does not leave old records unmanaged.

Before first apply, move the old per-host `cloudflare/ddns-dns-name` value to
shared `dns/local-prefix` in each used environment. Verify delivery before
removing the old entry. Host-specific multi-record DDNS template overrides
remain explicit and must reference their own documented leaves.

Design: [service network](../../../design/pihole-network.md).
