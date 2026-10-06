# Rupik to Clamps parity checklist

Status: source audit, 2026-10-06. This compares the repository's Chef include
path with current Skillet, Butane, and image definitions. It is not an inventory
of effective private Chef attributes or live Rupik state. Never copy production
domains, addresses, record names, zone IDs, tokens, or backups into this file.

Sources: `roles/rupik.rb` includes `role[server]`, `server::wol`, and `rupik`.
`site-cookbooks/rupik/recipes/default.rb` selects the application recipes below;
`server::default` supplies hardening, users, and Chef maintenance.

| Chef capability | Clamps implementation / deliberate change | Remaining work |
| --- | --- | --- |
| Btrfs `/srv` and system/user Podman stores | Shared `/var/lib/data` subvolume; system graphroot under `containers/system`, overlay driver on uCore | Final physical disk, encrypted backing filesystem, state transfer; rootless store only if a retained workload needs it |
| Disable resolved; Pi-hole | Mask/resolver setup, dual-stack port 53, persistent service data, encrypted password, shared bridge | Real LAN/firewall acceptance, actual custom DNS configuration, data transfer |
| nginx, ACME, OAuth proxy, web pod | Separate bridge containers; generic Caddy routes, Cloudflare DNS-01, tailnet restriction, per-service names and aliases | Production certificate/renewal and external-source denial; inventory any legacy cross-host routes still needed |
| Syncthing | Container and leaf data subvolume; peer ports retained; host GUI publication removed; explicit host/container identity mapping | PARKED: live ownership fix acceptance; production peer connectivity and identity/data migration |
| btrbk | Caller-selected Syncthing snapshots, hourly timer, retention and restore accepted | Off-machine backup policy and physical cutover acceptance; nested source subvolumes unsupported |
| UniFi | Rootful host-network container; numeric application data ownership; empty-controller/repeat-apply acceptance | Isolated backup restore, version compatibility, adoption and Site Manager access; see `CLAMPS-UNIFI.md` |
| Tailscale | Host network, persistent state, OAuth enrollment, disposable identity cleanup, DNS acceptance disabled | Chef advertises an exit node; current Skillet declaration does not. Add caller-controlled production exit-node behavior and prove forwarding/firewall/client routing |
| Cloudflare DDNS | Caller-selected rootful updater, validated secret config and production child-token delivery implemented; separate from UI DNS publication | Disposable lifecycle, live DNS acceptance, private record/proxy inventory and one-writer cutover; see `CLAMPS-CLOUDFLARE-DDNS.md` |
| Datadog | Retained by user decision, 2026-10-06; not implemented | Port metrics/process/network monitoring and retained checks; replace nginx-specific check as appropriate. Later local metrics/Grafana is separate work |
| EternalTerminal | Common image installs `et` and enables `et.service` | Physical service/listener/access validation |
| Wake-on-LAN | Common image installs `wol`; Chef emitted per-target helper scripts | Determine needed target shortcuts and supply MACs privately; package availability alone is not shortcut parity |
| Users, SSH, sudo, shell/dotfiles | Generic user environment/bootstrap and hardening exist; user acceptance previously passed | Compare additional Chef-managed accounts, permissions, and relevant hardening settings. Dotfiles install hook was explicitly deferred |
| Ubuntu package/Chef updates | Signed uCore images and Podman auto-update replace that maintenance mechanism | Physical OS update/rebase acceptance; no Chef daemon migration |
| Argon One | Omitted intentionally for x86_64 Clamps | None |

## Recipes outside the default path

`rupik::network` contains legacy NAT rules and a second Cloudflare update cron,
but `rupik::default` does not include it. Do not assume the cron/rules are absent
on the running host: inspect effective run lists and retained system state before
cutover. Do not port or enable a competing DNS writer merely because its source
exists. Additional private run-list recipes remain an inventory question.

## Work order

1. DDNS: the implementation-ready plan is the next service slice after the
   applicable refactor validation gate; do not mutate production DNS now.
2. Datadog port, then isolated UniFi backup/restore planning and execution.
3. Close exit-node and remaining host-baseline gaps before cutover.
4. Run the parked VM and deferred network/production acceptance when their
   required infrastructure is available; transfer state only with a recovery path.

Track migration ordering in `CLAMPS-MIGRATION.md`, refactor evidence in
`REFACTOR-FOLLOWUP.md`, and live results in `../butane/ACCEPTANCE.md`.
