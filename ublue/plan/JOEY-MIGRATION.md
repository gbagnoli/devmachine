# Boxy → joey

Status: host/boot slice implemented; native base readiness and fixture
repeat-apply/reboot accepted, 2026-10-11. Full application/provider acceptance
is pending a vault unlock; the rebuilt image publication and remote CI are unverified in this record.
Host identity changed to `joey` on 2026-10-11; the earlier native acceptance
used its previous identity. Fresh joey boot/application acceptance and publication
of `ghcr.io/gbagnoli/ucore-joey` remain pending. Existing VM manifests and vault
entries are not rewritten or accepted as joey implicitly.
Review completed against
`roles/boxy.rb` and `site-cookbooks/boxy/recipes/default.rb` and its includes.
Private Chef overrides and live boxy state have not been inventoried.
Clamps remaining work is tracked in [its migration plan](CLAMPS-MIGRATION.md).

## Source parity

| Boxy behavior | Target / remaining work |
| --- | --- |
| One Btrfs filesystem, separate data subvolume; rootful/rootless Podman stores | Shared root + `/var/lib/data`; rootful overlay graphroot; retain rootless storage only for workloads that need it |
| Pi-hole; resolved disabled | Selected shared Pi-hole and bridge DNS policy; image/day-zero mask; private runtime DNS delivery/v6 convergence and LAN acceptance pending |
| Syncthing data `/var/lib/data/sync`, dedicated account and ACLs | Shared `/var/lib/data/syncthing` leaf, declared host ownership/user mapping; transfer folder definitions, identity and ACLs explicitly, never copy live identity into smoke |
| btrbk snapshots `sync` only | Caller opts in for `syncthing` only; preserve leaf-only snapshots and settle production retention/off-machine backup |
| nginx/OAuth/ACME, local and cross-host routes | Generic Caddy, declared Pi-hole/Syncthing UIs and service alias policy; Cloudflare credentials and tailnet ingress reused; inventory needed cross-host routes before omitting them |
| Host-network Tailscale, exit node, LAN subnet advertisement, accept routes, SNAT disabled | Shared host-network persistent Tailscale/exit-node selection; **subnet advertisement, route acceptance and SNAT policy are not yet implemented** |
| Cloudflare DDNS | Shared selected updater/template; per-environment, per-host private record-name entry; one writer at production cutover |
| Datadog host/process/network checks and private cross-host probes | Shared selected Agent, production region `ftwo`, test `lab`/`env:test`; private remote probes not yet ported |
| ET and server baseline | Common image + shared hardening/user environment; shared SMTP also selected for every fleet server |

UniFi is absent from boxy's default recipe; joey does not select it.
No private addresses, domains, keys or effective Chef overrides belong here.

## Implementation order and exits

1. **Implemented; native boot/fixture accepted:** canonical `joey` crate/profile/image, shared
   full-apply unit, host Butane with existing `/dev/vda` and Btrfs layout.
   Use the new identity consistently without compatibility aliases.
   Profile selection must drive UIs, credentials, monitoring and snapshots;
   tests cover those declarations and bootstrap inputs. The matching image
   recipe must mask resolved. Validate strict Butane compilation, local CI
   and a fresh named VM's signed boot/base readiness. Real application and
   provider checks remain separate.
2. **Tailscale routing parity:** add explicit shared caller-selected routing
   policy and private delivery for subnet values. Preserve existing enrollment
   compatibility, converge preferences after authentication, and force disposable
   policy to omit advertisements. Test rendering, validation, repeat convergence
   and no production routes on smoke machines. Do not publish the LAN subnet.
3. **Private Pi-hole configuration:** replace static/sample DNS declarations
   with private runtime template delivery shared by both hosts. Use Pi-hole v6's
   supported configuration API/format; current `custom.list` rendering and the
   manual accepted `dns.hosts` test do not prove automatic v6 convergence. Validate
   empty/configured records, repeat apply, reboot and private-value redaction.
   Settle whether omission preserves restored records before production import.
4. **Named application acceptance:** unlock with the existing workflow; run
   joey full provisioning and capability-selected repeat-apply/reboot checks,
   then opted-in UI, DDNS, Datadog and isolated SMTP checks. Use disposable
   identities, test DNS and lifecycle-owned cleanup. No production writer/peer
   identity during acceptance; record failed and unavailable checks honestly.
5. **Private inventory and cutover:** inspect live boxy storage/subvolumes,
   Syncthing identity/folders/ACLs, Pi-hole settings, routing and cross-host
   monitoring/UI needs. Select real hardware/disk/root profile, snapshot/backup
   retention and rollback. Stage copies with original writers stopped as needed;
   do LAN/device and production certificate acceptance before moving addresses.

Use `skillet test vm create joey INSTANCE --port FREE_PORT`, then
`test vm ready`, `test vm provision`, `test smoke --with-applications` and
`test vm destroy` through the shared Rust lifecycle owner. Provisioning may
need cached vault access; do independent work if unlock would block. Install
nothing on the workstation. Commit focused changes after validation; do not push.

Evidence: [ACCEPTANCE.md](archive/ACCEPTANCE.md).
