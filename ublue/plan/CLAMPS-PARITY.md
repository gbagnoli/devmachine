# Rupik to Clamps parity checklist

Status: source audit updated 2026-10-09; baseline VM observations recorded in
`CLAMPS-BASELINE-AUDIT.md`. This compares the repository's Chef include
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
| UniFi | Rootful host-network container; numeric application data ownership; empty-controller/repeat-apply, isolated backup restore, restored-data persistence and operator UI inspection accepted | Production device adoption and Site Manager access at cutover; SMTP setup parked; see `CLAMPS-UNIFI.md` |
| Tailscale | Host network, persistent state, OAuth enrollment, disposable identity cleanup, DNS acceptance disabled; caller-selected production exit-node advertisement implemented, disabled for disposable VMs | Approve the production exit node in tailnet policy and select it on clients; live routing exercise omitted by operator decision; see `../design/tailscale.md` |
| Cloudflare DDNS | Caller-selected rootful updater, clear shared template with KeePassXC leaf references, production delivery, and journaled disposable token/record cleanup implemented; separate from UI DNS publication | Explicit rotation acceptance and private record/proxy inventory and one-writer cutover; see `CLAMPS-CLOUDFLARE-DDNS.md` |
| Datadog | Retained; approved rootful Agent, service-owned labels, host checks and credential delivery implemented | Live host/container checks, submissions and automatic boot discovery passed; NPM/USM perf/HTTP traffic and reboot acceptance passed with explicit PERFMON; physical Secure Boot, provider tag read-back and richer application checks pending; see `CLAMPS-DATADOG.md`. Later local metrics/Grafana is separate work |
| EternalTerminal | Common image installs `et`; VM unit active/enabled and dual-stack port 2022 listening | Physical firewall and end-to-end client access |
| Wake-on-LAN | Common image installs `wol`; Chef emitted per-target helper scripts | Determine needed target shortcuts and supply MACs privately; package availability alone is not shortcut parity |
| Users, SSH, sudo, shell/dotfiles | SSH syntax/effective configuration accepted; uCore supplies passwordless sudo; Brew and dotfiles installer completed, with Vim plugins explicitly skipped | UID 1001 accepted; explicit post-provisioning sudo/password-expiry finalization implemented, live interactive login pending; review production keys; broader OS-hardening function remains a placeholder. See `CLAMPS-BASELINE-AUDIT.md` |
| Ubuntu package/Chef updates | Signed uCore images and Podman auto-update replace that maintenance mechanism | Physical OS update/rebase acceptance; no Chef daemon migration |
| Argon One | Omitted intentionally for x86_64 Clamps | None |

## Recipes outside the default path

`rupik::network` contains legacy NAT rules and a second Cloudflare update cron,
but `rupik::default` does not include it. Do not assume the cron/rules are absent
on the running host: inspect effective run lists and retained system state before
cutover. Do not port or enable a competing DNS writer merely because its source
exists. Additional private run-list recipes remain an inventory question.

## Work order

1. Disposable DDNS publication/recovery/cleanup passed; finish explicit rotation
   acceptance and privately inventory production records;
   do not mutate production DNS.
2. Datadog eBPF and isolated UniFi restore/UI acceptance passed. Shared SMTP
   delivery to Gmail also passed; UniFi email setup is parked. Retain physical
   Secure Boot, provider tag read-back and richer monitoring follow-ups in
   `CLAMPS-DATADOG.md`.
3. Production exit-node configuration is implemented; local policy checks passed.
   Next: resolve account/bootstrap policy and reconcile broader OS hardening
   using the ordered follow-ups in `CLAMPS-BASELINE-AUDIT.md`; audit completed
   without configuration changes.
4. Run the parked VM and deferred network/production acceptance when their
   required infrastructure is available; transfer state only with a recovery path.

Track migration ordering in `CLAMPS-MIGRATION.md`, refactor evidence in
`REFACTOR-FOLLOWUP.md`, and live results in `../butane/ACCEPTANCE.md`.
