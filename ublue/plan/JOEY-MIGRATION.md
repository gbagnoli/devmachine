# Joey: replacement hardware and service rehearsal

Status: source/profile review refreshed 2026-10-16. Joey currently uses a
Raspberry Pi 5; replacement hardware has not been chosen. Prepare the complete
service profile in a named smoke VM now; hardware selection and physical cutover
are separate later gates. The existing x86_64 image/installer does not establish
Raspberry Pi support. ARM installation is outside this plan.

The host profile, binary, Butane and image recipe exist. Earlier base/fixture
acceptance predates the rename; fresh joey signed boot, full application/provider
acceptance and matching image publication remain unverified in this record.
Existing manifests/vault entries are not implicitly migrated. Boxy's Chef
role/recipes are historical inventory: reconcile them with current Raspberry Pi
services before production migration. Private overrides/live state remain uninspected.

## Source parity

| Boxy behavior | Target / remaining work |
| --- | --- |
| One Btrfs filesystem, separate data subvolume; rootful/rootless Podman stores | Shared root + `/var/lib/data`; rootful overlay graphroot; retain rootless storage only for workloads that need it |
| Pi-hole; resolved disabled | Selected shared Pi-hole and bridge DNS policy; image/day-zero mask; shared encrypted v6 custom DNS delivery implemented; joey-specific settings migration and DNS/LAN acceptance pending |
| Mikochi file browsing | Already included with Syncthing, identity-protected with no app login; joey-specific file operations and isolation acceptance pending |
| Syncthing data `/var/lib/data/sync`, dedicated account and ACLs | Shared `/var/lib/data/syncthing` leaf, declared host ownership/user mapping; transfer folder definitions, identity and ACLs explicitly, never copy live identity into smoke |
| btrbk snapshots `sync` only | Caller opts in for `syncthing` only; preserve leaf-only snapshots and settle production retention/off-machine backup |
| nginx/OAuth/ACME, local and cross-host routes | Generic Caddy, declared Pi-hole/Syncthing UIs and service alias policy; Cloudflare credentials and tailnet ingress reused; inventory needed cross-host routes before omitting them |
| Host-network Tailscale, exit node, LAN subnet advertisement, accept routes, SNAT disabled | Shared host-network persistent Tailscale/exit-node selection; **subnet advertisement, route acceptance and SNAT policy are not yet implemented** |
| Cloudflare DDNS | Shared selected updater/template; joey will use a distinct record from clamps; configure the host override and verify independent ownership |
| Datadog host/process/network checks and private cross-host probes | Shared selected Agent, production region `ftwo`, test `lab`/`env:test`; private remote probes not yet ported |
| ET and server baseline | Common image + shared hardening/user environment; shared SMTP also selected for every fleet server |

Native static networking is not selected for joey: select the existing shared
module once replacement interfaces/addressing are known. Do not copy clamps'
UPS, UniFi or optional Home Assistant selections.

Clamps and joey will each update a different DDNS record. Keep the shared
environment default for callers that use it and configure joey's distinct name
through its host template override/private leaf. Verify each updater owns only
its intended record; the shared default alone does not implement this decision. Static Pi-hole DNS inventory
does not synchronize gravity, groups or leases. Add settings synchronization only
if the current source actually uses it and it is explicitly retained.

UniFi is absent from boxy's default recipe; joey does not select it.
No private addresses, domains, keys or effective Chef overrides belong here.

## Implementation order and exits

1. **Existing baseline; fresh joey acceptance pending:** canonical `joey` crate/profile/image, shared
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
   policy to omit production advertisements and route acceptance. Test rendering,
   validation, repeat convergence and no production routes on smoke machines. Do not publish the LAN subnet.
3. **Private Pi-hole configuration:** shared encrypted v6 delivery is implemented
   using environment `dns/local-prefix` and `dns/entries`. Complete private
   inventory migration and joey-specific runtime/production acceptance. The DNS
   phase owns the complete custom-host list; container-only apply leaves it
   untouched. Empty inventory removes managed entries. Preserve source state
   before import and validate repeat apply/reboot and actual client answers.
4. **Complete named service VM before hardware purchase:** use host `joey`
   and a separate instance such as `services`, with a free SSH port. Verify image
   publication and fresh signed boot, update current binaries, audit test secrets,
   and provision all selected services with synthetic data and independent identities.
   Explicitly opt into UI/private DNS, disposable DDNS and Datadog checks; SMTP
   uses isolated capture unless a real test recipient is authorized. Default
   application smoke omits some services: run their owning module checks too.

   **Exit checklist:**

   - Base readiness, Btrfs data mount, users, SSH/sudo/ET and binary identity pass.
   - Every selected unit starts; repeated provisioning/apply and reboot preserve state.
   - Pi-hole answers synthetic A/AAAA records on TCP/UDP; repeated/empty DNS
     delivery preserves unrelated v6 settings.
   - Syncthing transfers harmless data only between disposable peers. Mikochi
     upload/read/delete preserves ownership; source identities are never reused.
   - Trusted HTTPS and browser/API flows work for an allowed human-owned tailnet
     client. Tagged/denied clients, forged headers, identity lookup outages and
     direct backend access fail closed. A tagged VM cannot prove human authorization.
   - btrbk snapshots only the selected leaf and restores a sample into isolated state.
   - Datadog receives test/lab metrics without production paging; SMTP capture passes.
   - Synthetic routing checks pass; no production LAN routes, networking credentials,
     source peers or Pi-hole replication are activated in smoke.
   - Owned DNS/token/node cleanup passes. Keep the VM for iteration if useful,
     then recreate once to prove fresh provisioning; record unavailable checks honestly.

5. **Replacement hardware and physical rehearsal:** choose architecture, capacity,
   Ethernet and TPM support; verify corresponding image/installer support. Existing
   x86_64 plain-root/TPM profiles are reusable, but VM acceptance does not prove
   physical firmware/drivers or TPM behavior. Keep virtual `/dev/vda` and physical
   disk inputs separate. Inventory live storage/subvolumes, Syncthing folders/ACLs,
   Pi-hole settings, routes and private monitoring/UI needs; settle source-to-target
   paths, account mapping, snapshot retention, off-machine backup and rollback.

   **Exit:** disk/root profile selected, recoverable backups verified, native
   networking staged and explicitly activated. Real LAN clients, resolver failover,
   subnet return paths/approvals, exit-node access and remote management pass.
   These gates do not block the full-service smoke VM.

6. **Production cutover:** stop relevant source writers for the final consistent
   copy. Import real Syncthing identity/folders and Pi-hole settings only into the
   intended production host. Activate joey's distinct DDNS record and production
   monitoring, validate DNS/private UIs/routing, and preserve the source for rollback.
   Finalize password/sudo policy after provisioning completes. Never snapshot all
   `/var/lib/data` or synchronize raw database clusters as a backup strategy.

Follow the [VM guide](../skillet/crates/vm/README.md) with host `joey` and a
separate instance. Run `update` after `ready` to restore current binaries before
provisioning; readiness can reinstall captured bootstrap artifacts. Use the shared
Rust lifecycle owner for creation, provisioning, smoke and destruction. Provisioning may
need cached vault access; do independent work if unlock would block. Install
nothing on the workstation. Commit focused changes after validation; do not push.

Evidence: [ACCEPTANCE.md](archive/ACCEPTANCE.md).
