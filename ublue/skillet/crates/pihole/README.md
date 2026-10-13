# Pi-hole

The caller must declare Pi-hole. Run Cargo commands from `ublue/skillet`.

## Credentials and apply

Production uses `skillet/hosts/<host>/pihole/web-password` in KeePassXC Password.
For a fresh host with no installed credential/Podman secret, Skillet generates
and saves a missing entry. For an existing host, restore its original vault
entry rather than inventing a replacement. Close KeePassXC before generated saves.

```sh
cargo run --release -p skillet -- secrets deliver HOST pihole --target USER@ADDRESS
```

This stages only. Add `--activate` to run the full-apply consumer, which also
configures other declared applications. [Remote apply](../workstation/README.md#apply-a-production-host)
stages both Pi-hole and Tailscale first. Test VMs get a generated disposable
password via `test vm provision HOST INSTANCE`; repeat provision reuses it,
`--rotate` replaces it. No production password reaches a smoke VM.

If a credential is missing/undecryptable, redeliver the vault entry. For a corrupt
disposable credential, use `--rotate`. Do not replace a production value with an
empty/default password.

## Service and storage

Pi-hole publishes host IPv4/IPv6 port 53 over TCP and UDP. The UI is private on
bridge port 8088 through [Caddy](../caddy/README.md), not published on the host.
Persistent configuration/logs are under `/var/lib/data/pihole/{etc,log}`.
Inspect `pihole.service` and its journal. Preserve a working bootstrap resolver
while Pi-hole is down; follow [native networking](../network/README.md) for DNS.

The current recipe's custom-record input is a static declaration writing
`custom.list`; do not treat that as accepted private Pi-hole v6 configuration.
See the [migration tracker](../../../plan/CLAMPS-MIGRATION.md) for that gap.

Design: [service network](../../../design/pihole-network.md).
