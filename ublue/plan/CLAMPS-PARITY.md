# Rupik to Clamps parity checklist

Status: documentation reconciled 2026-11-02; earlier baseline observations are in
[the archived audit](archive/CLAMPS-BASELINE-AUDIT.md). This compares the Chef include
path with current Skillet, Butane, and image definitions. It is not an inventory
of effective private Chef attributes or live Rupik state. Never copy production
domains, addresses, record names, zone IDs, tokens, or backups into this file.

Sources: `roles/rupik.rb` includes `role[server]`, `server::wol`, and `rupik`.
`site-cookbooks/rupik/recipes/default.rb` selects the application recipes below;
`server::default` supplies hardening, users, and Chef maintenance.

| Chef capability | Clamps implementation / deliberate change | Remaining work |
| --- | --- | --- |
| Btrfs `/srv` and system/user Podman stores | Shared `/var/lib/data` subvolume; system graphroot under `containers/system`, overlay driver on uCore | Fresh replacement 1 TB encrypted install and signed bootstrap passed; physical cold power-on operator-confirmed; current-volume independent recovery and update/rollback checks remain; no old stock-SSD state transfer requested |
| Disable resolved; Pi-hole | Mask/resolver setup, dual-stack port 53, persistent service data, identity-gated isolated web backend and encrypted shared v6 DNS inventory | Shared DNS inventory delivered; explicit IPv4 LAN queries passed. Verify intended replacement-installation records, DHCP-client use and outage behavior; no old Pi-hole state import assumed |
| nginx, ACME, OAuth proxy, web pod | Separate bridge containers; generic Caddy routes, Cloudflare DNS-01, tailnet restriction, per-service names and aliases | Production certificate/renewal and external-source denial; inventory any legacy cross-host routes still needed |
| Syncthing | Container and leaf data subvolume; peer ports retained; host GUI publication removed; explicit host/container identity mapping | Operator paired replacement Clamps; verify actual peer receipt/off-machine backups. Flatpak ownership-fix acceptance remains parked independently |
| btrbk | Caller-selected Syncthing snapshots, hourly timer, retention and restore accepted | Off-machine backup policy and physical cutover acceptance; nested source subvolumes unsupported |
| UniFi | Rootful host-network container; numeric application data ownership; empty-controller/repeat-apply, isolated backup restore, restored-data persistence and operator UI inspection accepted | Restored on replacement Clamps; operator sees both APs. Verify sustained adoption/WLANs, Site Manager, reboot and off-machine backup; SMTP parked; see `CLAMPS-UNIFI.md` |
| Tailscale | Host network, persistent state, OAuth enrollment, disposable identity cleanup, DNS acceptance disabled; caller-selected production exit-node advertisement implemented, disabled for disposable VMs | Approve the production exit node in tailnet policy and select it on clients; live routing exercise omitted by operator decision; see `../design/tailscale.md` |
| Cloudflare DDNS | Caller-selected rootful updater, clear shared template with KeePassXC leaf references, production delivery, and journaled disposable token/record cleanup implemented; separate from UI DNS publication | Production one-writer handover and strict repeat delivery passed; rotation, private inventory and next-reboot publication remain; see `CLAMPS-CLOUDFLARE-DDNS.md` |
| Datadog | Rootful Agent, module-owned checks, credentials and lab/test tag isolation; native VM discovery/eBPF/reboot accepted; NUT collector implemented | Physical Secure Boot Agent/cloud ingestion passed; cold-boot recovery, NUT/UPS acceptance and optional richer checks remain; [monitoring plan](CLAMPS-DATADOG.md) |
| EternalTerminal | Common image installs `et`; physical IPv4/IPv6 tailnet TCP, authenticated ET command and wired/Wi-Fi IPv4 LAN TCP passed; shared baseline derives private LAN grants; image startup convergence/repeat verified in a VM | Deploy updated Skillet for automatic LAN convergence; verify rebuilt-image drop-in and baseline at next planned physical boot |
| Wake-on-LAN | Common image installs `wol`; Chef emitted per-target helper scripts | Determine needed target shortcuts and supply MACs privately; package availability alone is not shortcut parity |
| Users, SSH, sudo, shell/dotfiles | SSH syntax/effective configuration accepted; provisioning uses temporary passwordless sudo, explicit finalization enables password-required sudo; Brew/dotfiles installer accepted, Vim plugins explicitly skipped | UID difference accepted; operator finalized sudo. Verify updated deployment on the finalized host; shared sudo cache passed VM acceptance. Review production keys; broader OS hardening remains a placeholder. See [the archived baseline audit](archive/CLAMPS-BASELINE-AUDIT.md) |
| Ubuntu package/Chef updates | Signed uCore images and Podman auto-update replace that maintenance mechanism | Earlier physical signed rebase/reboot accepted; verify replacement-installation update/rollback and preservation. Automatic reboot remains disabled; no Chef daemon migration |
| Argon One | Omitted intentionally for x86_64 Clamps | None |
| Static Ethernet/Wi-Fi (new requirement) | Caller-selected private vault fields, encrypted delivery and boot-only profile preparation | Image/profiles implemented; earlier physical wired/Wi-Fi/reboot tests passed. Verify replacement-installation details, DNS outages and management recovery; [network plan](HOST-NETWORKING.md) |

## Recipes outside the default path

`rupik::network` contains legacy NAT rules and a second Cloudflare update cron,
but `rupik::default` does not include it. Do not assume the cron/rules are absent
on the running host: inspect effective run lists and retained system state before
cutover. Do not port or enable a competing DNS writer merely because its source
exists. Additional private run-list recipes remain an inventory question.

## Work order

Follow [clamps migration](CLAMPS-MIGRATION.md) for deployment/cutover sequencing.
Keep [refactor acceptance](SKILLET-REFACTOR.md) separate from production scopes.
Replacement 1 TB installation and application delivery are complete. Do not
repeat stock-SSD migration/finalization; physical cold power-on is confirmed. Keep current-volume recovery,
off-machine backup and remaining service checks separate from bootstrap success.
Record actual results in [acceptance](archive/ACCEPTANCE.md).
