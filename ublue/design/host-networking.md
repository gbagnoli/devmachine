# Native host networking

Callers select wired and/or Wi-Fi interfaces. Private interface names, static
IPv4 settings, resolver order and Wi-Fi credentials live in structured KeePassXC
entries. Other hosts and disposable VMs retain DHCP; physical network delivery
is production-only.

The workstation validates all fields and subnet relationships before delivery.
A versioned, host-bound `network_config` payload uses verified SSH stdin and
host-encrypted systemd credentials. Root-only NetworkManager profiles are
regenerated in `/run/NetworkManager/system-connections`; persistent plaintext
Wi-Fi keys are unnecessary. Payloads and rendered buffers zeroize on drop.
The network phase rejects diagnostic recording.

Base apply installs a generic preparation unit before NetworkManager and after
local filesystems. It has no network-online or application-data dependency.
Missing credentials skip preparation. A preparation failure leaves NetworkManager
able to use retained operator/DHCP profiles.

Preparation never reloads or activates connections. Fresh profiles prepared on a
running host have autoconnect disabled; boot preparation enables them before
NetworkManager starts. Later live preparation preserves the autoconnect flag of
managed boot profiles. Ethernet uses route metric 100, Wi-Fi 600. Managed profiles
have higher autoconnect priority than default DHCP; existing profiles are retained
for recovery. Wi-Fi uses its permanent MAC. DNS order is preserved exactly;
DHCP/RA DNS is ignored while IPv6 addresses remain automatic.

Live connection switching/checkpoint rollback is outside this boot-preparation
contract. See
[setup](../skillet/crates/network/README.md) and
[acceptance plan](../plan/HOST-NETWORKING.md).

Reference: [NetworkManager keyfiles](https://networkmanager.pages.freedesktop.org/NetworkManager/NetworkManager/nm-settings-keyfile.html).
