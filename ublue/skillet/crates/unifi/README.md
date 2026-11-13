# UniFi Network

The caller opts in to a rootful host-network controller. It stores data at
`/var/lib/data/unifi`, with the image's `unifi` process UID/GID 999. Only the
volume root gets those declared numeric IDs; existing descendants are preserved.
No matching host account is required.

Use [remote apply](../workstation/README.md#apply-a-production-host). Inspect
`unifi.service`, its journal and the `unifi` container health. Remote access uses
UniFi Site Manager at `unifi.ui.com`; there is no Caddy route. Controller/UI
accounts are managed inside UniFi, not Skillet's vault.

## Device firewall access

Apply ensures TCP 8080 (inform), UDP 3478 (STUN), and UDP 10001 (discovery)
in both runtime and permanent firewalld configuration. Sources are the private
IPv4 subnets of active hardware/virtio NICs, in each NIC's current firewall zone.
Container bridges, VPNs, public subnets and the admin port 8443 are excluded.
The host must have firewalld and an active directly attached RFC1918 LAN.
No private addresses are compiled into the binary.

Convergence adds missing rules without a firewall reload or controller restart.
It preserves existing rules; opting out or moving networks does not remove old
grants automatically. Remove obsolete grants explicitly when retiring a network.
Check device reachability from another LAN machine: a local listener check alone
does not establish that the host firewall permits adoption traffic.

## Restore and inspect

Use a compatible `.unf` backup through the setup/UI flow. Keep backups private
and outside version control. Do not copy ARM MongoDB files to the x86_64 target.
Stop the old production controller before enabling its replacement.

For a disposable restore, follow the [isolated restore procedure](docs/restore.md).
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

## Backup replication

Configure native automatic backups in UniFi. Clamps currently uses Thursday
01:30, retaining seven copies. Skillet's `skillet-unifi-backup.timer` exports
stable native backups daily at 03:30 Europe/Paris. It does not create backups.

Source: `/var/lib/data/unifi/data/backup/autobackup/`.
Destination: `backups/unifi/` in the caller's shared Syncthing files root; on
Clamps this is `/var/lib/data/syncthing/data/backups/unifi/`.
Seven newest exported copies are retained; unrelated files and source backups
are preserved. Publication uses private files readable by the declared Syncthing
owner. Allow at least one minute after backup creation before exporting.

After booting an image containing the exporter, run:

```bash
sudo systemctl start skillet-unifi-backup.service
sudo journalctl -u skillet-unifi-backup.service --no-pager -n 30
systemctl list-timers skillet-unifi-backup.timer
```

For a direct retry:

```bash
sudo /usr/bin/skillet unifi export
```

Check that the `.unf` appears on a receiving Syncthing peer. Missing backups,
content conflicts and unsafe source/destination paths fail without pruning.

## Home Assistant startup readiness

On hosts selecting both applications, HA waits for
`skillet-unifi-ready.service` before starting. The helper polls the controller's
JSON `/status` endpoint for up to three minutes, rather than treating an open
port or startup HTML as readiness. Failure is logged, but HA still starts after
the bounded check. UniFi is never restarted by this gate.

The helper ships as `ghcr.io/gbagnoli/skillet-unifi-ready:latest`: a non-root
static Rust/musl executable in a scratch image. Its publishing workflow must
complete before the first rollout; ensure the GHCR package is public so hosts
can pull without registry credentials. PR builds do not publish; the default branch
and daily schedule publish fresh images. The gate checks for a newer helper on
startup and can reuse a cached image if the registry is unavailable.

After booting the updated uCore image:

```bash
sudo systemctl start skillet-unifi-ready.service
journalctl -u skillet-unifi-ready.service --no-pager -n 30
systemctl show homeassistant.service -p After -p Wants
```

A healthy run ends successfully; the oneshot returns inactive so a future HA
startup performs a fresh check. Installing the dependency does not restart an
already-running HA or reload its UniFi integration. For an existing stalled
integration, use its Reload action in HA. Do not reboot just to test readiness.
