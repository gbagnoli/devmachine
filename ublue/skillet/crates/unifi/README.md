# UniFi Network

The caller opts in to a rootful host-network controller. It stores data at
`/var/lib/data/unifi`, with the image's `unifi` process UID/GID 999. Only the
volume root gets those declared numeric IDs; existing descendants are preserved.
No matching host account is required.

Use [remote apply](../workstation/README.md#apply-a-production-host). Inspect
`unifi.service`, its journal and the `unifi` container health. Remote access uses
UniFi Site Manager at `unifi.ui.com`; there is no Caddy route. Controller/UI
accounts are managed inside UniFi, not Skillet's vault.

## Restore and inspect

Use a compatible `.unf` backup through the setup/UI flow. Keep backups private
and outside version control. Do not copy ARM MongoDB files to the x86_64 target.
Stop the old production controller before enabling its replacement.

For a disposable restore, follow the [isolated restore procedure](../../../plan/CLAMPS-UNIFI.md).
Install and verify the persistent test network fence **before** importing live
state; do not enable cloud access or change device inform addresses there.
From `ublue/skillet`, open its local UI with the owned forward:

```sh
cargo run --release -p skillet -- test vm ssh HOST INSTANCE --forward 18443:8443
```

Open `https://127.0.0.1:18443`; the self-signed certificate does not match localhost.
For a workstation that is itself remote, forward its port 18443 through an
additional SSH connection from your browser machine.

The service-only fixture builds with
`cargo build --release -p skillet_unifi --example acceptance_apply`. Deploy its
reported artifact through the owned VM transport; it neither imports data nor
configures unrelated applications. Imported accounts/passwords come from the
backup. SMTP recovery-email setup is not part of this operating flow.

Design: [UniFi](../../../design/unifi.md).
