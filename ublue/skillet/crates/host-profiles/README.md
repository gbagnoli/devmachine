# Host declarations and module guides

[src/profile.rs](src/profile.rs) owns caller-selected capabilities. Generic and
host-specific binaries use the same declaration. `clamps`, `donbot` and `joey`
are full profiles; `agent` is the credential-free baseline. `/etc/skillet/host`
selects the installed profile independently of a disposable VM's runtime
hostname.

Syncthing/file browsing, Homer, and SMTP are shared fleet policy. Optional capabilities
must be selected explicitly; vault entries do not enable them. Runtime values
stay in the vault or private installation inputs.

| Module | Operating guide |
| --- | --- |
| Snapshots | [btrbk](../btrbk/README.md) |
| Private HTTPS / identity | [Caddy](../caddy/README.md) |
| Monitoring | [Datadog](../datadog/README.md) |
| Public-address DNS updates | [DDNS](../ddns/README.md) |
| File browsing | [Mikochi](../mikochi/README.md) |
| Service dashboard | [Homer](../homer/README.md) |
| Static Ethernet / Wi-Fi | [Networking](../network/README.md) |
| UPS and metrics | [NUT](../nut/README.md) |
| DNS | [Pi-hole](../pihole/README.md) |
| Joplin Server | [Joplin](../joplin/README.md) |
| Jellyfin | [Jellyfin](../jellyfin/README.md) |
| Plex | [Plex](../plex/README.md) |
| File sharing | [Gokapi](../gokapi/README.md) |
| AirTrail | [AirTrail](../airtrail/README.md) |
| Arr / putioarr | [Media](../media/README.md) |
| Mail relay | [SMTP](../smtp/README.md) |
| Home automation | [Home Assistant](../homeassistant/README.md) |
| PostgreSQL | [PostgreSQL](../postgres/README.md) |
| Synchronization | [Syncthing](../syncthing/README.md), including Donbot's personal instance |
| Tailnet enrollment / routing | [Tailscale](docs/tailscale.md) |
| Controller | [UniFi](../unifi/README.md) |
| Vault / remote apply | [Workstation](../workstation/README.md) |
| Root encryption / recovery | [TPM](../workstation/docs/tpm.md) |
| Disposable testing | [VM](../vm/README.md) |
| Sudo / first-login password | [Guest access](../cli-common/README.md) |

Design: [fleet ownership](../../../design/fleet-hosts.md).
Migration trackers: [plans](../../../plan/README.md).
