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
3. Retain the old controller stopped and independent backup/rollback copies;
   confirm the desired off-machine backup policy.

Disposable repeats use the [isolated restore procedure](../skillet/crates/unifi/docs/restore.md)
and must remain fenced before any production-state import. Production adoption
and cloud registration are excluded from that test. SMTP password recovery
remains [parked](CLAMPS-EMAIL.md#parked), not a cutover gate.
[Design](../design/unifi.md).
