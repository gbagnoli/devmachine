# SSH and Eternal Terminal

SSH TCP 22 is source-unrestricted by the base firewall policy and listens on
IPv4/IPv6. Public-key authentication remains required. Use `ssh HOST` normally;
`ssh USER@LAN_ADDRESS` is a DNS/Tailscale-independent recovery path on the LAN.

Use `et HOST` or `et USER@LAN_ADDRESS` for Eternal Terminal. The image installs
and enables `et.service` and allows TCP 2022 from standard Tailscale ranges.
Shared Skillet base apply additionally derives private IPv4 LAN subnet grants
from active hardware NIC addresses/netmasks, including configured Ethernet and
Wi-Fi. No separate subnet secret is required. Public-only hosts skip LAN grants;
container bridges and VPNs are excluded. LAN IPv6 ET grants are not implemented.

The resource updates permanent and runtime rules without a firewall reload or ET
restart. It preserves existing grants; remove obsolete grants explicitly when
retiring a network. Inspect `et.service` and verify client access from the actual
LAN/tailnet. A local listener check alone does not establish firewall access.
The image's English locale package supports `en_US.UTF-8` remote shells.

Design: [administrative access](../../../../design/administrative-access.md).
