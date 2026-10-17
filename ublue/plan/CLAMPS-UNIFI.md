# UniFi production cutover

Rootful host-network deployment, empty-controller acceptance, isolated `.unf`
UI restore, repeated apply/reboot persistence, operator authenticated inspection
and disposable cleanup have [evidence](archive/ACCEPTANCE.md).
[Historical restore record](archive/CLAMPS-UNIFI.md).
Current operation: [UniFi guide](../skillet/crates/unifi/README.md).

## Remaining

1. Obtain a current compatible backup and effective source application version
   privately. Keep independent backup/rollback copies; do not copy ARM MongoDB
   data into the x86_64 target or commit restored settings/accounts.
2. Schedule cutover with the old controller stopped. Restore only after data-root
   UID/GID 999 and management reachability are correct. Ordinary Skillet apply
   must not import/overwrite an application database.
3. Verify production device discovery/adoption, sites/networks/WLANs and inform
   addresses before enabling Site Manager. Do not reset/forget devices merely
   because a restored inventory shows them offline.
4. Verify local management, cloud access, repeat apply and unattended reboot,
   then retain backups/off-machine protection according to the agreed policy.

Disposable repeats use the [isolated restore procedure](../skillet/crates/unifi/docs/restore.md)
and must remain fenced before any production-state import. Production adoption
and cloud registration are excluded from that test. SMTP password recovery
remains [parked](CLAMPS-EMAIL.md#parked), not a cutover gate.
[Design](../design/unifi.md).
