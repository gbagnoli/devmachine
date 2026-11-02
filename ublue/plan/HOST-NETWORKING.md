# Native host networking

## Implemented

Caller-selected wired/Wi-Fi profiles; structured vault fields; format, host and
subnet validation; encrypted production delivery; separate guest preparation;
boot ordering; volatile root-only keyfiles; DNS-order preservation. Clamps opts
in to both interfaces. Other hosts and smoke VMs retain DHCP. Remote all-service
apply prepares networking without switching the active connection. See
[design](../design/host-networking.md).

## Physical acceptance status

Wired/Wi-Fi provisioning, wireless connectivity, route preference, reboot and
Wi-Fi switching have operator/inspection evidence from the stock-SSD installation.
The replacement 1 TB install completed full credential/application delivery on
2026-10-31; it is no longer waiting for hardware installation. Earlier NIC checks
must not silently stand in for replacement-installation observations. See
[acceptance evidence](archive/ACCEPTANCE.md).

## Remaining physical checks

1. Verify current prepared runtime profiles, permanent-MAC matching, static
   addresses, route preference and encrypted input on the replacement installation.
   Use the existing preparation/activation boundary; do not log profiles or PSKs
   or switch the management connection during a remote check.
2. At a supervised boot, verify preparation before NetworkManager, DNS ordering,
   local Pi-hole and independent DNS availability before Pi-hole starts.
3. Check normal client DHCP-provided resolver use and Pi-hole outage behavior.
   Explicit queries from Rupik and earlier route/switch tests are already recorded.
4. With recovery access available, confirm replacement-installation management
   through Ethernet/Wi-Fi and independent LAN SSH if Tailscale is unavailable.
   Preserve earlier acceptance; do not disable management remotely to prove it.

Implementation and disposable ordering/repeat tests are completed work. Their
previous rollout checklist is [archived](archive/CLAMPS-BEFORE-SSD-RECONCILIATION.md).

Offline parser validation and injected tests do not establish physical wireless
association or actual reboot reachability. Record evidence separately in
`plan/archive/ACCEPTANCE.md`. Live switching/checkpoint rollback remains future work.

Physical binary updates now use `skillet deploy HOST --target USER@HOST`;
remote `apply` performs that deployment by default. Network-only delivery still
requires running the guest base phase once to install the preparation unit.
