# Clamps migration draft

Prerequisite: complete [the Skillet refactoring roadmap](SKILLET-REFACTOR.md)
before proceeding with the remaining feature milestones in this plan.
Existing acceptance evidence and deferred tasks remain applicable.

Status: reviewed against source and recorded acceptance, 2026-10-11.
Shared services, native TPM recovery and physical USB media preparation are
implemented. Clamps is not production-ready until the physical/cutover gates
below pass. The parked Flatpak/Syncthing and deferred network checks remain
open; previous passing native VM checks do not close them.

## What remains for clamps

1. Boot the prepared USB on the S13; inspect firmware/TPM and select its stock
   SSD explicitly. Enroll the trusted MOK when using Secure Boot. Validate the
   selected plain or TPM profile on hardware, recovery archive and cold boot.
2. Supply private production DNS/DDNS records and keys, migrate service data
   and identities from rupik, and decide retained cross-host routes and WOL
   targets. Keep the old DNS service and Samsung SSD until the cutover.
3. Validate real LAN IPv4/IPv6 UDP/TCP DNS, peer connectivity, physical SSH/ET,
   UniFi devices/cloud access, production ACME/renewal and outside-tailnet denial.
   Approve the production exit node; routed-traffic checks remain omitted by
   operator decision. Finish automated delivery before interactive access
   finalization and first-login password-change acceptance.
4. Verify physical repeat apply, reboot and OS update; take service-data backups,
   choose off-machine backup retention, rehearse rollback and agree the cutover.
   Finish or explicitly defer the outstanding boot-policy/transport-interruption
   tests. Broader OS hardening is still a placeholder, not an accepted port.

No new clamps service crate is required to start
[boxy → beelzebot](BEELZEBOT-MIGRATION.md). Acceptance details remain in
[ACCEPTANCE.md](../butane/ACCEPTANCE.md) and the service plans.

## Goal and starting point

Provision clamps with uCore + Skillet and migrate the required behavior and persistent state of the Chef-managed rupik. Complete one milestone at a time; add reusable Skillet resources when a service needs them.

Implemented: base/full apply separation, activation recovery, fixture smoke
checks, temporary VM SSH keys, binary delivery, resolver configuration, signed
bootstrap, user environment, exact KeePassXC reads, and encrypted Pi-hole
credential delivery, and the shared Btrfs data layout. A dummy-credential
Pi-hole container runs in the VM with data and Podman storage under
`/var/lib/data`. Shared service networking, test Cloudflare lifecycle, and
staging ACME/HTTPS have passed named-VM acceptance. Final physical disk,
production/network acceptance, and remaining rebase/bootstrap checks stay open.

The [source parity checklist](CLAMPS-PARITY.md) records retained behavior and
deliberate changes. [Cloudflare DDNS](CLAMPS-CLOUDFLARE-DDNS.md) has its guest
service, production delivery, and disposable ownership/cleanup implementation.
Live disposable DNS publication and cleanup passed; rotation and production
cutover remain separate. Datadog live acceptance, isolated UniFi backup restore
and operator UI inspection passed. Shared SMTP delivery to Gmail passed;
UniFi email setup is parked by operator decision.

After the refactoring prerequisite: complete Pi-hole LAN acceptance and configure
the actual custom DNS
records once their values are supplied. On 2026-10-02, the running smoke VM
passed IPv4/IPv6 UDP/TCP DNS queries and a reversible custom-record test.
Its libvirt user-mode network forwards only SSH, so client ingress and firewall
behavior remain unverified; use a bridged/physical clamps network for that
check. Keep production DNS on rupik. Generic private UI staging acceptance is
recorded in [ACCEPTANCE.md](../butane/ACCEPTANCE.md); two deferred UI checks are
tracked in [generic private UI provisioning](GENERIC-PRIVATE-UIS.md).
Syncthing passed the named disposable-VM service and repeat-apply checks, and
the [btrbk milestone](CLAMPS-BTRBK.md) is complete.
See the [private UI design](../design/private-ui-access.md), [Syncthing
design](../design/syncthing.md), [Pi-hole network design](../design/pihole-network.md),
[storage design](../design/storage.md), [secret design](../design/secrets.md),
[VM lifecycle](../design/smoke-vms.md), and
[storage/credential plan](CLAMPS-STORAGE-CREDENTIALS.md). The final physical
disk remains open.

## Deferred live acceptance batch

Run these together when a suitable client network and the required production
configuration are available; record each result in
[ACCEPTANCE.md](../butane/ACCEPTANCE.md):

1. Pi-hole: query from a real LAN client over IPv4/IPv6 and UDP/TCP, confirming
   host-firewall behavior. Use isolated test records; add actual custom DNS
   records only after their values are supplied.
2. Private UIs: verify a request from outside the tailnet gets Caddy's denial
   response. Workstation loopback is not an outside-tailnet source.
3. Cloudflare production: accept production certificate issuance and a bounded
   renewal scenario using the production environment and its existing DNS.

The smoke VM uses libvirt user-mode networking and cannot satisfy item 1; use a
bridged test VM or the physical clamps host on an isolated/test network.

Native [TPM-encrypted Btrfs root](TPM-ENCRYPTED-ROOT.md), QEMU TPM2 tests
and recovery are implemented and accepted. Physical media generation is
implemented; stock-SSD hardware acceptance remains required before cutover. Tang is
deferred; remote hosts will first be assessed for usable TPM support.

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
- Physical USB media generation now selects a whole-disk by-id identifier and
  reuses the shared root-profile renderer; VM creation retains `/dev/vda`. Follow
  [the hardware runbook](../butane/HARDWARE.md). Actual disk selection, firmware
  checks, MOK enrollment and stock-SSD installation remain unverified. Fresh
  native `/dev/vda` regression reached signed boot and full readiness after
  transient bootstrap/SSH retries; evidence is in the acceptance record.

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
- The Chef DNS listener behavior is represented as wildcard IPv4/IPv6 TCP/UDP port 53 publication. The running smoke VM answers queries on its IPv4/IPv6 interface addresses, but its `passt` user-mode network only forwards SSH; verify ingress from a real LAN client through the host firewall before accepting LAN reachability.
- Pi-hole password-file consumption and startup identity/permissions passed on the VM. Pi-hole v6 accepted and served a temporary `dns.hosts` custom record; the setting was restored empty. Configure actual custom records only after their values are supplied.
- Replace placeholder DNS records with supplied configuration. Support multiple names per address.
- Keep host bootstrap DNS independent of Pi-hole availability. Verify IPv4/IPv6 TCP and UDP DNS, web authentication, persistence, rotation, repeated apply and reboot.
- Decide whether Nebula Sync is required and which instance is authoritative before enabling it.

Done when: clients can use the isolated clamps instance and all required Pi-hole configuration survives restarts. Production DNS remains on rupik. Network Quadlet unit tests pass, and the VM has confirmed service-name web access and host-port DNS publication; remaining network and persistence checks are listed above.

## Milestone 5: migrate one remaining service at a time

Each row is a separate implementation and validation step. Adjust order for dependencies; do not implement all rows as one batch.

| Order | Capability | Behavior and state to preserve |
| --- | --- | --- |
| 1 | Syncthing (implemented; ownership correction committed; live retry parked) | Data, device identity, folder configuration, UID/GID, required ports; production peer and state transfer remain |
| 2 | btrbk (implemented and verified) | Follow the [btrbk milestone](CLAMPS-BTRBK.md): real Btrfs subvolume, hourly snapshots, Chef retention policy, and restore exercise. Local snapshots are not an independent backup. |
| 3 | UniFi (implemented; isolated restore and operator inspection passed) | Rootful host-network container and persistent data accepted; production devices/cloud access remain cutover checks. See [UniFi port plan](CLAMPS-UNIFI.md). No Caddy route. |
| 4 | Tailscale (implemented; smoke enrollment and cleanup accepted) | Host network, state, OAuth enrollment, and cleanup exist. Caller-selected production exit-node advertisement is implemented, disabled for smoke VMs. Existing forwarding policy is retained; approve the production device in tailnet policy. Live routing exercises are omitted by operator decision. UI names use Cloudflare DNS-only records. |
| 5 | Private UI access with Caddy + ACME (implemented; staging accepted) | Generic declarations/aliases, encrypted delivery, scoped account tokens, A/AAAA/CNAME reconciliation and cleanup exist. Staging ACME/HTTPS passed. Production renewal, real-client resolution and separate-network denial remain; see [UI plan](GENERIC-PRIVATE-UIS.md). |
| 6 | Cloudflare DDNS | Guest service, production delivery, and journaled disposable lifecycle implemented; live disposable publication/cleanup passed; explicit rotation and production one-writer handover remain separate; see [DDNS plan](CLAMPS-CLOUDFLARE-DDNS.md) |
| 7 | Datadog and remaining host baseline | Datadog implementation and live VM acceptance passed. Baseline audit and access finalization are implemented; physical checks remain in the [parity checklist](CLAMPS-PARITY.md) |

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
- Services: actual DNS/DDNS records, retained cross-host routes, exit-node operation,
  Nebula Sync, and legacy networking. Datadog is retained and its live VM acceptance passed.
- Cutover: address/identity transitions, outage window and rollback duration.

## Technical references

- [systemctl waiting semantics](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml)
- [Quadlet enablement](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html#enabling-unit-files)
- [Podman secret drivers and replacement](https://docs.podman.io/en/latest/markdown/podman-secret-create.1.html)
- [Pi-hole configuration and password files](https://docs.pi-hole.net/docker/configuration/)
