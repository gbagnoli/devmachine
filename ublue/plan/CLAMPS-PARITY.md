# Rupik to Clamps parity checklist

Status: documentation reconciled 2026-10-13; earlier baseline observations are in
[the archived audit](archive/CLAMPS-BASELINE-AUDIT.md). This compares the Chef include
path with current Skillet, Butane, and image definitions. It is not an inventory
of effective private Chef attributes or live Rupik state. Never copy production
domains, addresses, record names, zone IDs, tokens, or backups into this file.

Sources: `roles/rupik.rb` includes `role[server]`, `server::wol`, and `rupik`.
`site-cookbooks/rupik/recipes/default.rb` selects the application recipes below;
`server::default` supplies hardening, users, and Chef maintenance.

| Chef capability | Clamps implementation / deliberate change | Remaining work |
| --- | --- | --- |
| Btrfs `/srv` and system/user Podman stores | Shared `/var/lib/data` subvolume; system graphroot under `containers/system`, overlay driver on uCore | Stock-SSD encrypted install/boot evidence exists; final-disk/update/recovery acceptance and state transfer remain; rootless storage only for a retained workload |
| Disable resolved; Pi-hole | Mask/resolver setup, dual-stack port 53, persistent service data, identity-gated isolated web backend and encrypted shared v6 DNS inventory | Production vault/record migration, real LAN/firewall acceptance, data transfer |
| nginx, ACME, OAuth proxy, web pod | Separate bridge containers; generic Caddy routes, Cloudflare DNS-01, tailnet restriction, per-service names and aliases | Production certificate/renewal and external-source denial; inventory any legacy cross-host routes still needed |
| Syncthing | Container and leaf data subvolume; peer ports retained; host GUI publication removed; explicit host/container identity mapping | PARKED: Flatpak ownership-fix acceptance; production peer connectivity and identity/data migration |
| btrbk | Caller-selected Syncthing snapshots, hourly timer, retention and restore accepted | Off-machine backup policy and physical cutover acceptance; nested source subvolumes unsupported |
| UniFi | Rootful host-network container; numeric application data ownership; empty-controller/repeat-apply, isolated backup restore, restored-data persistence and operator UI inspection accepted | Production device adoption and Site Manager access at cutover; SMTP setup parked; see `CLAMPS-UNIFI.md` |
| Tailscale | Host network, persistent state, OAuth enrollment, disposable identity cleanup, DNS acceptance disabled; caller-selected production exit-node advertisement implemented, disabled for disposable VMs | Approve the production exit node in tailnet policy and select it on clients; live routing exercise omitted by operator decision; see `../design/tailscale.md` |
| Cloudflare DDNS | Caller-selected rootful updater, clear shared template with KeePassXC leaf references, production delivery, and journaled disposable token/record cleanup implemented; separate from UI DNS publication | Explicit rotation acceptance and private record/proxy inventory and one-writer cutover; see `CLAMPS-CLOUDFLARE-DDNS.md` |
| Datadog | Rootful Agent, module-owned checks, credentials and lab/test tag isolation; native VM discovery/eBPF/reboot accepted; NUT collector implemented | Physical Secure Boot, NUT timer/SELinux/cloud ingestion, provider tag read-back and richer checks; [monitoring plan](CLAMPS-DATADOG.md) |
| EternalTerminal | Common image installs `et`; VM unit active/enabled and dual-stack port 2022 listening | Physical firewall and end-to-end client access |
| Wake-on-LAN | Common image installs `wol`; Chef emitted per-target helper scripts | Determine needed target shortcuts and supply MACs privately; package availability alone is not shortcut parity |
| Users, SSH, sudo, shell/dotfiles | SSH syntax/effective configuration accepted; uCore supplies passwordless sudo; Brew and dotfiles installer completed, with Vim plugins explicitly skipped | UID 1001 accepted; explicit post-provisioning sudo/password-expiry finalization implemented, live interactive login pending; review production keys; broader OS-hardening function remains a placeholder. See [the archived baseline audit](archive/CLAMPS-BASELINE-AUDIT.md) |
| Ubuntu package/Chef updates | Signed uCore images and Podman auto-update replace that maintenance mechanism | Physical OS update/rebase acceptance; no Chef daemon migration |
| Argon One | Omitted intentionally for x86_64 Clamps | None |
| Static Ethernet/Wi-Fi (new requirement) | Caller-selected private vault fields, encrypted delivery and boot-only profile preparation | Image support, physical address/association adoption and management recovery; [network plan](HOST-NETWORKING.md) |

## Recipes outside the default path

`rupik::network` contains legacy NAT rules and a second Cloudflare update cron,
but `rupik::default` does not include it. Do not assume the cron/rules are absent
on the running host: inspect effective run lists and retained system state before
cutover. Do not port or enable a competing DNS writer merely because its source
exists. Additional private run-list recipes remain an inventory question.

## Work order

Follow [clamps migration](CLAMPS-MIGRATION.md) for deployment/cutover sequencing.
Keep [refactor acceptance](SKILLET-REFACTOR.md) separate from production scopes.
Stock-S13 signed boot/TPM evidence does not accept final-disk installation,
first-login password change, live network/firewall or service-data migration.
Record actual results in [acceptance](archive/ACCEPTANCE.md).
