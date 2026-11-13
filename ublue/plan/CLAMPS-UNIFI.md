# UniFi production cutover

Rootful host-network deployment, empty-controller acceptance, isolated `.unf`
UI restore, repeated apply/reboot persistence, operator authenticated inspection
and disposable cleanup have [evidence](archive/ACCEPTANCE.md).
[Historical restore record](archive/CLAMPS-UNIFI.md).
Current operation: [UniFi guide](../skillet/crates/unifi/README.md).

## Production cutover status

A fresh compatible backup was restored on physical Clamps. LAN firewall grants
were added in runtime and permanent configuration without restarting UniFi;
the operator confirms both access points now appear in its UI. LAN TCP inform
reachability was checked from Rupik over both host interfaces; the management
port remains blocked from that client. No private addresses are stored here.

The UniFi module now declares its device ports and converges source-scoped
firewalld grants through the shared resource boundary. Ordinary apply preserves
application data and never reloads the whole firewall.

## Remaining

1. Confirm both access points remain adopted, WLANs function and inform addresses
   continue pointing to the replacement controller. Do not reset/forget devices
   merely because a restored inventory shows them offline.
2. Verify local management and Site Manager, then unattended reboot persistence
   at an operator-selected maintenance point. Persistent rule configuration is
   checked separately from actual reboot acceptance.
3. Verify scheduled native backups and exporter replication on the current host.
   Operator selected Thursday 01:30 with seven native copies. The implemented
   daily 03:30 exporter retains seven copies under the shared Syncthing files
   root (`backups/unifi`). Confirm initial export, receiving-peer receipt and
   restore from that independent copy after deploying the updated image. Retain
   independent backup/rollback copies.

Disposable repeats use the [isolated restore procedure](../skillet/crates/unifi/docs/restore.md)
and must remain fenced before any production-state import. Production adoption
and cloud registration are excluded from that test. SMTP password recovery
remains [parked](CLAMPS-EMAIL.md#parked), not a cutover gate.
[Design](../design/unifi.md).

## HA startup readiness rollout

The operator reported simultaneous phone/person `not_home` states after reboot.
Read-only logs confirm HA login requests received startup HTML from the local
controller even though UniFi's systemd service was already active. Later status
queries report ready JSON. This supports an API-readiness race but does not prove
that every later presence problem has the same cause.

Implemented: Rust/musl scratch readiness container, on-demand bounded systemd
unit, conditional HA Wants/After composition and first-apply provider ordering.
No watchdog or automatic controller/HA restart is introduced. Local tests,
container startup/timeout fixtures and a read-only probe of the live controller
have passed; no production service activation or reboot has been performed.

Remaining: push and verify the helper-image workflow; make its GHCR package
public; release Skillet/image definitions; verify HA startup after controller
readiness in a named disposable VM and the next supervised physical boot.
Verify timeout allows HA startup while leaving UniFi running. Reload an already
stalled UniFi config entry through HA separately; installing startup ordering
does not reload it. Do not initiate another physical kexec/reboot for this test.
