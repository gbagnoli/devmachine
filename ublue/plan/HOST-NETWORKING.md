# Native host networking

## Implemented

Caller-selected wired/Wi-Fi profiles; structured vault fields; format, host and
subnet validation; encrypted production delivery; separate guest preparation;
boot ordering; volatile root-only keyfiles; DNS-order preservation. Clamps opts
in to both interfaces. Other hosts and smoke VMs retain DHCP. Remote all-service
apply prepares networking without switching the active connection. See
[design](../design/host-networking.md).

## Acceptance before physical activation

1. Install matching generic and host binaries using the existing deployment
   paths. Verify the deployed image has NetworkManager Wi-Fi support and a
   supplicant. Add missing packages to the image repository, not the workstation.
2. Deliver networking with explicit preparation using the README command.
   Confirm root-only runtime profiles and encrypted persistent input. Fresh live
   profiles must have autoconnect disabled; current addresses and management
   connectivity must remain unchanged. Do not log profile contents or PSKs.
3. First exercise ordering and repeat preparation in an isolated disposable VM,
   with fixture inputs and disposable NICs, preserving the management NIC. Never
   deliver production network entries to test VMs.
4. With local-console recovery available, reboot physical hardware. Verify
   preparation precedes NetworkManager, static Ethernet address, requested DNS
   order, local Pi-hole over loopback and independent DNS before Pi-hole startup.
5. Verify Wi-Fi association/static address/permanent MAC with Ethernet attached
   and detached, route preference with both active, management connectivity,
   repeated preparation and another unattended boot.

Offline parser validation and injected tests do not establish physical wireless
association or actual reboot reachability. Record evidence separately in
`plan/archive/ACCEPTANCE.md`. Live switching/checkpoint rollback remains future work.

Physical binary updates now use `skillet deploy --host HOST --target USER@HOST`;
remote `apply` performs that deployment by default. Network-only delivery still
requires running the guest base phase once to install the preparation unit.
