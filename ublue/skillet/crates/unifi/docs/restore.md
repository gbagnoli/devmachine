# Isolated UniFi restore procedure

Run Cargo commands from `ublue/skillet`. Use a dedicated named disposable VM;
keep normal smoke instances and the production controller separate.

1. Keep the `.unf` backup in a private directory outside version control. Record
   source version/date privately and retain an independent copy.
2. Create the owned test VM, record the compatible target version/digest and
   fetch its image before fencing. Stop UniFi before importing any state.
3. Install the VM-only [fence service](../../../../butane/tests/fixtures/unifi-restore/fence.service),
   [rules](../../../../butane/tests/fixtures/unifi-restore/fence.nft) and
   [UniFi drop-in](../../../../butane/tests/fixtures/unifi-restore/unifi-dropin.conf)
   through the owned transport. Preserve loopback, management SSH and required
   DHCP/IPv6-neighbour traffic; deny other IPv4/IPv6 ingress/egress and disable
   automatic image updates. NAT alone is not a fence.
4. On the empty guest, arm a short recovery rollback only while checking fresh
   SSH connectivity. Cancel it before enabling the controller or importing the
   backup. Never leave a timer that can reconnect restored production state.
   Verify denied LAN/tailnet/public egress through rules and counters before
   import and after reboot; an unreachable probe alone is insufficient.
5. Build the service-only fixture with
   `cargo build --release -p skillet_unifi --example acceptance_apply`. Deploy
   Cargo's reported artifact via the guarded VM transport and apply as root on
   the prepared guest. It does not restore state or provision other services.
6. Use the supported `.unf` setup/UI import through
   `skillet test vm ssh HOST INSTANCE --forward 18443:8443`. Do not guess a
   restore API or send account passwords in arguments. Importing the backup
   restores its accounts. Device offline status is expected in the fence.
7. Inspect expected sites/networks/WLANs/device totals, local HTTPS, host-network
   mode and data-root UID/GID 999. Repeat the isolated module apply, then reboot
   through the VM owner and verify the fence precedes controller startup and
   restored state persists. Keep full-host acceptance distinct.
8. Stop the restored controller and destroy the owned VM through the normal
   lifecycle. Preserve backup copies and private diagnostics until inspection
   succeeds. Never enable cloud registration or alter production inform settings
   in this isolated run.

Record observations in [acceptance](../../../../plan/archive/ACCEPTANCE.md).
Production cutover is in [the plan](../../../../plan/CLAMPS-UNIFI.md).
