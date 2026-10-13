# Private file browsing

Syncthing hosts run Mikochi at `https://files.<host>.<ui-domain>`. With the default
prefix the name is `files.<host>.ui.<zone-domain>`.

## Setup

Configure [Caddy identity protection](../caddy/README.md#identity-protected-uis)
and [Tailscale access](../host-profiles/docs/tailscale.md). From `ublue/skillet`:

```sh
cargo run --release -p skillet -- secrets check --host HOST --environment test
cargo run --release -p skillet -- test vm provision HOST INSTANCE --with-ui
```

Open the files hostname from an allowed person's untagged tailnet client.
There is no additional Mikochi password or public host port. Redeliver Caddy
credentials after changing users or the caller's UI inventory.

The default file scope is `/var/lib/data/syncthing/data`, with the same declared
owner/namespace mapping as Syncthing. Configure synchronized folders under that
root. New Syncthing installations default to `data/Sync`; retained folder paths
must be configured/moved explicitly. Syncthing's `config` directory and other
service data are outside this browser's scope.

Inspect `mikochi.service`, `skillet-tailnet-ui.service` and `caddy.service`.
Do not publish the backend port or bypass its isolated Caddy network.

Designs: [file browsing](../../../design/file-browsing.md),
[tailnet identity](../../../design/tailnet-ui-authentication.md).
