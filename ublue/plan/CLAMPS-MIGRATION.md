# Clamps migration draft

Status: remaining work, 2026-09-27. Skillet convergence and disposable VM
bootstrap are implemented; their completed agent assignments have been removed.
Runtime evidence and outstanding bootstrap checks are recorded in
[ACCEPTANCE.md](../butane/ACCEPTANCE.md).

## Goal and starting point

Provision clamps with uCore + Skillet and migrate the required behavior and persistent state of the Chef-managed rupik. Complete one milestone at a time; add reusable Skillet resources when a service needs them.

Implemented: base/full apply separation, activation recovery, fixture smoke
checks, temporary VM SSH keys, binary delivery, resolver configuration, signed
bootstrap, user environment, exact KeePassXC reads, and encrypted Pi-hole
credential delivery, and the shared Btrfs data layout. A dummy-credential
Pi-hole container runs in the VM with data and Podman storage under
`/var/lib/data`; service networking, live ACME, final disk selection, and
post-provision rebase checks remain open.

Next: implement [generic private UI provisioning](GENERIC-PRIVATE-UIS.md),
using host-declared services and environment domains from KeePassXC, then
validate Cloudflare DNS-01 staging for private Pi-hole and Syncthing UIs.
The private-UI design requires a tailnet-only
listener and DNS-only Cloudflare records pointing to tailnet addresses, so
certificate issuance alone is not the acceptance target.
Pi-hole LAN-client reachability and final custom DNS records also remain open;
production DNS stays on rupik. Syncthing passed the named disposable-VM service
and repeat-apply checks, and the [btrbk milestone](CLAMPS-BTRBK.md) is complete.
See the [private UI design](../design/private-ui-access.md), [Syncthing
design](../design/syncthing.md), [Pi-hole network design](../design/pihole-network.md),
[storage design](../design/storage.md), [secret design](../design/secrets.md),
[VM lifecycle](../design/smoke-vms.md), and
[storage/credential plan](CLAMPS-STORAGE-CREDENTIALS.md). The final physical
disk remains open.

## Boundaries

- Image: packages, static OS files, baseline unit enablement/masking. Preserve common-before-host image builds.
- Ignition: installation layout, initial access and bootstrap configuration.
- Workstation Skillet: KeePassXC reads and guarded entry creation, a three-hour
  kernel unlock cache, and secret delivery over SSH after base readiness.
- Skillet: continuing convergence of host values, application configuration, secrets, storage resources and containers.
- Public repositories and images must exclude secret values, including secrets compiled into host binaries. Decide which topology values also require private delivery.
- Keep the production Samsung SSD and functioning DNS on rupik until cutover. Test on the stock clamps SSD or disposable VM disks.
- Leave desktop `ublue/tailscale/` and `ublue/llama/` outside this work.
- Preserve Rust safety/error-handling rules and idempotency. Replace the blanket `systemctl --wait` rule with waiting for startup job completion: `--wait` waits for service termination and can hang on persistent services.

## Remaining bootstrap verification

- Exercise interrupted bootstrap and capture resolver continuity across intermediate boots.
- Reconcile the acceptance log's repeatability status with its recorded second fresh VM; do not infer unrecorded checks passed.
- Confirm automatic base apply before readiness explicitly restarts the unit, and collision refusal for unrelated domains/disks.
- Prepare physical disk selection and hardware/Secure Boot/MOK enrollment separately from the VM-only `/dev/vda` template; validate on the stock SSD.

## Milestone 3: storage and credentials for the first application

Resolve only the decisions needed for Pi-hole first.

- The shared `/var/lib/data` mount and Pi-hole persistence path are implemented
  and accepted on a disposable VM. Chef used `/srv/pihole` on rupik.
- Put formatting/partition creation in the installation path. Skillet must not format existing data during convergence.
- Skillet and the managed Pi-hole service now depend on the real data mount;
  the missing and wrong-mount VM checks passed.
- Directory/subvolume/mount checks and system Podman's graphroot under
  `/var/lib/data` passed fresh-VM acceptance. Merge the shared Butane storage
  include when adding each future host template.
- Extend the implemented direct KeePassXC read and SSH delivery to other named credentials as services need them. Disposable Pi-hole passwords are generated in VM tests.
- A signed image update and explicit digest-pinned rebase preserved the encrypted credential; post-rebase readiness and full apply passed. Recovery after atomic credential installation followed by failed full-apply activation also passed. A transport interruption during streaming/encryption and production TPM binding remain open. Atomic replacement, rotation, repeated apply and reboot passed in the VM.
- Account for Podman's on-disk secret copy in the production disk-protection decision.

Done when: Pi-hole's storage and password can be provisioned, recovered after interruption, and rotated without leaking values into repository files, command arguments or logs. Reboot/rebase recovery works. The persistent-data mount failure case is exercised.

## Milestone 4: working Pi-hole on an isolated test address

- Implemented the reusable Quadlet network resource. Pi-hole now has a dual-stack bridge, network DNS enabled, container name `pihole`, and registry auto-update; see the [network design](../design/pihole-network.md).
- VM apply found Aardvark DNS colliding with Pi-hole's host port 53. Skillet now configures rootful Aardvark DNS to listen on port 54. On the named uCore VM, Pi-hole starts, Aardvark resolves its dual-stack container addresses from another container, an HTTP request reaches `http://pihole:8088/admin/`, and Pi-hole answers DNS over UDP and TCP through host IPv4 and IPv6 addresses. `ss` confirms host publication on IPv4 and IPv6 for both transports.
- Replace the Chef web pod with separate containers on the shared bridge. Caddy can resolve `pihole` and proxy to port 8088 without static container IPs or a host-published admin port.
- The Chef DNS listener behavior is represented as wildcard IPv4/IPv6 TCP/UDP port 53 publication. Verify the test VM's host firewall and actual client reachability before considering this item accepted.
- Pi-hole password-file consumption and startup identity/permissions passed on the VM. Verify custom-record format for the selected image version and DNS reachability from a LAN client through the host firewall.
- Replace placeholder DNS records with supplied configuration. Support multiple names per address.
- Keep host bootstrap DNS independent of Pi-hole availability. Verify IPv4/IPv6 TCP and UDP DNS, web authentication, persistence, rotation, repeated apply and reboot.
- Decide whether Nebula Sync is required and which instance is authoritative before enabling it.

Done when: clients can use the isolated clamps instance and all required Pi-hole configuration survives restarts. Production DNS remains on rupik. Network Quadlet unit tests pass, and the VM has confirmed service-name web access and host-port DNS publication; remaining network and persistence checks are listed above.

## Milestone 5: migrate one remaining service at a time

Each row is a separate implementation and validation step. Adjust order for dependencies; do not implement all rows as one batch.

| Order | Capability | Behavior and state to preserve |
| --- | --- | --- |
| 1 | Syncthing (implemented; disposable VM service and repeat-apply checks passed) | Data, device identity, folder configuration, UID/GID, required ports |
| 2 | btrbk (implemented and verified) | Follow the [btrbk milestone](CLAMPS-BTRBK.md): real Btrfs subvolume, hourly snapshots, Chef retention policy, and restore exercise. Local snapshots are not an independent backup. |
| 3 | UniFi | Controller data or supported backup restore, version compatibility, adoption and ownership |
| 4 | Tailscale (implemented; smoke enrollment and cleanup accepted 2026-09-30) | Host-network container, persistent state, KeePassXC OAuth client, one-use tagged key delivery, and smoke device cleanup are implemented. Production forwarding remains to validate. UI names use Cloudflare DNS-only records; Tailscale DNS management is not required. |
| 5 | Private UI access with Caddy + ACME (implementation started) | Caddy's credential-gated Quadlet config proxies the Pi-hole and Syncthing UIs by bridge DNS name; Syncthing's host-published GUI port is removed. Next wire smoke-specific KeePassXC values, mint per-VM zone-scoped tokens and manage DNS-only A/AAAA records through the workstation Cloudflare token creator. Validate tailnet-only access, ACME staging, certificate persistence/renewal, record/token cleanup, repeated runs and interruption recovery. |
| 6 | Cloudflare DDNS | Required records and token delivery; reconcile the legacy updater before enabling competing writers |
| 7 | Monitoring and remaining host baseline | Explicit keep/drop decision for Datadog; required hardening, users, SSH/sudo, ET and WOL behavior |

For each service: inspect effective Chef inputs, settle its open decisions, add only needed Skillet support, migrate a copy of state, validate functionality and repeat convergence. Account for ARM-to-x86 application/image and data compatibility. Avoid activating duplicate production identities or DNS writers during testing.

Syncthing validation used an empty disposable data subvolume, creating a VM-only
device identity. `syncthing` resolved to both bridge addresses, its UI returned
HTTP 200 by container name, all Chef-published dual-stack ports were present,
and a repeated full apply preserved its container ID and config hash. Testing
connectivity with a real peer and migrating rupik's identity/data remain
cutover tasks; never run both copies of the production device identity at once.

Legacy NAT rules and the separate updater live in a recipe that is not included by rupik's default recipe. Confirm whether they are active or still required before porting them. Argon One support is Pi-specific and excluded.

## Milestone 6: cut over with a recovery path

- Reconcile the parity checklist against effective production configuration and explicitly record intentional omissions.
- Record the final disk/data transfer approach; take usable backups and perform final synchronization with stateful writers stopped where necessary.
- Transfer chosen addresses, DNS/DHCP settings, names and service identities without running conflicting instances.
- Exercise client DNS, synchronization, UniFi, VPN/exit-node operation, proxy authentication, certificate renewal, DDNS and restores as applicable.
- Verify a reboot, an OS update/rebase and a repeated Skillet apply on the target.
- Retain a documented way to return service to rupik until clamps has passed the agreed observation period.

Done when: all retained capabilities and data work on clamps, routine convergence and updates work, and the user accepts retirement of rupik's production role.

## Decisions deferred until needed

- Storage: stock-SSD test layout, final disk arrangement, and post-provision
  OS rebase acceptance. The disposable VM mount-failure and graphroot checks
  passed.
- Credentials: production TPM/rebase policy and protection of Podman's on-disk copy; portable storage and delivery are decided in the secret design.
- Services: actual DNS records, domains/routes, Tailscale enrollment, Nebula Sync, Datadog and legacy networking.
- Cutover: address/identity transitions, outage window and rollback duration.

## Technical references

- [systemctl waiting semantics](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml)
- [Quadlet enablement](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html#enabling-unit-files)
- [Podman secret drivers and replacement](https://docs.podman.io/en/latest/markdown/podman-secret-create.1.html)
- [Pi-hole configuration and password files](https://docs.pi-hole.net/docker/configuration/)
