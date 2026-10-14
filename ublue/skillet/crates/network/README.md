# Native host networking

Run Cargo commands from `ublue/skillet`.

Clamps selects Ethernet and Wi-Fi; other callers remain on DHCP until opted in.
Disposable VMs retain DHCP. Create:

- `skillet/hosts/<host>/network/wired`: Username/Password unused.
- `skillet/hosts/<host>/network/wifi`: Username = SSID; Password = personal WPA
  key (8–63 printable ASCII bytes or 64 hex digits).

Both entries require custom fields `interface`, `address`, `mask`, `gateway`,
`dns1`, `dns2`. Use IPv4 addresses and an interface name from `ip link`. The mask
can be a prefix (`24` or `/24`) or a contiguous dotted mask. Gateway and static
address must share a subnet. Use distinct interfaces/addresses. Resolver order
is preserved; loopback can address local Pi-hole, with an independent resolver
available while Pi-hole is down.

Run `skillet secrets check --host HOST --environment production`. Install matching
generic and host binaries first. Run the guest base phase once to install the
preparation unit, then deliver and prepare only networking:

```sh
ssh USER@CURRENT_ADDRESS sudo -n /var/usrlocal/bin/skillet apply \
  --host-file /etc/skillet/host --phase base
cargo run --release -p skillet -- secrets deliver HOST network \
  --target USER@CURRENT_ADDRESS --activate
```

`--activate` runs the preparation unit only: it never restarts/reloads
NetworkManager or changes live addresses. Remote `apply HOST --target ...`
also prepares networking in the full workflow.

Profiles are root-only under `/run/NetworkManager/system-connections`; persistent
input is encrypted in `/etc/credstore.encrypted/skillet/network_config.cred`.
Fresh live profiles have autoconnect disabled. On the next boot they are recreated
with autoconnect enabled before NetworkManager starts. Existing boot profiles
preserve autoconnect during later live preparation. Ethernet routes are preferred
over Wi-Fi. DHCP profiles are retained. `--phase network --record` is rejected.

Before physical reboot, verify image Wi-Fi support and keep local-console access
available. Reconnect on the new address or Tailscale. Check
`journalctl -b -u skillet-network-prepare.service`, `ip address`, `ip route` and
`/etc/resolv.conf`. Avoid printing keyfiles. Follow the
[acceptance plan](../../../plan/HOST-NETWORKING.md) and
[design](../../../design/host-networking.md).
