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

The current recipe's custom-record input is a static declaration writing
`custom.list`; do not treat that as accepted private Pi-hole v6 configuration.
See the [migration tracker](../../../plan/CLAMPS-MIGRATION.md) for that gap.

Design: [service network](../../../design/pihole-network.md).
