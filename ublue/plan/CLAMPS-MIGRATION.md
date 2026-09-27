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
credential delivery. A dummy-credential Pi-hole container runs in the VM;
service networking, live ACME, and production storage remain open.

Next: implement Pi-hole storage and isolated service networking, following the
[secret design](../design/secrets.md), [VM lifecycle](../design/smoke-vms.md), and
[storage design](../design/storage.md) and
[storage/credential plan](CLAMPS-STORAGE-CREDENTIALS.md). The selected storage
direction is a shared Btrfs filesystem with `/var/lib/data` as the data mount;
the final physical disk remains open.

## Boundaries

- Image: packages, static OS files, baseline unit enablement/masking. Preserve common-before-host image builds.
- Ignition: installation layout, initial access and bootstrap configuration.
- Workstation Skillet: KeePassXC reads and secret delivery over SSH after base readiness.
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

- Implement the decided `/var/lib/data` mount and Pi-hole persistence path.
  Chef currently uses `/srv/pihole`; Skillet currently uses `/etc/pihole`.
- Put formatting/partition creation in the installation path. Skillet must not format existing data during convergence.
- Make applications depend on their real data mount so a missing disk cannot silently produce an empty replacement data directory on the OS filesystem.
- Add directory/subvolume/ownership support and place system Podman's graphroot
  under `/var/lib/data` on fresh installs, as decided in the storage design.
- Extend the implemented direct KeePassXC read and SSH delivery to other named credentials as services need them. Disposable Pi-hole passwords are generated in VM tests.
- Verify interruption recovery after credential delivery and credential survival across an additional OS rebase. Host-key encryption, atomic replacement, rotation, repeat apply and reboot passed in the VM. Confirm production TPM binding against actual hardware.
- Account for Podman's on-disk secret copy in the production disk-protection decision.

Done when: Pi-hole's storage and password can be provisioned, recovered after interruption, and rotated without leaking values into repository files, command arguments or logs. Reboot/rebase recovery works. The persistent-data mount failure case is exercised.

## Milestone 4: working Pi-hole on an isolated test address

- Add the dual-stack network resource, explicit container attachment and auto-update configuration.
- Decide whether to retain the Chef web pod or publish Pi-hole ports independently. Keep its admin endpoint compatible with the planned Caddy proxy.
- Pi-hole password-file consumption and startup identity/permissions passed on the VM. Verify SELinux labels, DNS publication, upstreams, web port and custom-record format for the selected image version.
- Replace placeholder DNS records with supplied configuration. Support multiple names per address.
- Keep host bootstrap DNS independent of Pi-hole availability. Verify IPv4/IPv6 TCP and UDP DNS, web authentication, persistence, rotation, repeated apply and reboot.
- Decide whether Nebula Sync is required and which instance is authoritative before enabling it.

Done when: clients can use the isolated clamps instance and all required Pi-hole configuration survives restarts. Production DNS remains on rupik.

## Milestone 5: migrate one remaining service at a time

Each row is a separate implementation and validation step. Adjust order for dependencies; do not implement all rows as one batch.

| Order | Capability | Behavior and state to preserve |
| --- | --- | --- |
| 1 | Syncthing | Data, device identity, folder configuration, UID/GID, required ports |
| 2 | btrbk | Real Btrfs subvolumes, hourly snapshots, Chef retention policy, restore exercise; local snapshots are not an independent backup |
| 3 | UniFi | Controller data or supported backup restore, version compatibility, adoption and ownership |
| 4 | Tailscale | Authentication/identity policy, exit-node approval, forwarding, persistent state, DNS policy and hardware interface settings |
| 5 | Caddy evaluation + OAuth2 proxy + ACME | Port required nginx domains/routes and access rules; validate Cloudflare DNS-01 with staging, per-VM tokens, certificate persistence and cleanup |
| 6 | Cloudflare DDNS | Required records and token delivery; reconcile the legacy updater before enabling competing writers |
| 7 | Monitoring and remaining host baseline | Explicit keep/drop decision for Datadog; required hardening, users, SSH/sudo, ET and WOL behavior |

For each service: inspect effective Chef inputs, settle its open decisions, add only needed Skillet support, migrate a copy of state, validate functionality and repeat convergence. Account for ARM-to-x86 application/image and data compatibility. Avoid activating duplicate production identities or DNS writers during testing.

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

- Storage: stock-SSD test layout, final disk arrangement, mount-failure
  acceptance, and graphroot relocation.
- Credentials: production TPM/rebase policy and protection of Podman's on-disk copy; portable storage and delivery are decided in the secret design.
- Services: actual DNS records, domains/routes, Tailscale enrollment, Nebula Sync, Datadog and legacy networking.
- Cutover: address/identity transitions, outage window and rollback duration.

## Technical references

- [systemctl waiting semantics](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml)
- [Quadlet enablement](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html#enabling-unit-files)
- [Podman secret drivers and replacement](https://docs.podman.io/en/latest/markdown/podman-secret-create.1.html)
- [Pi-hole configuration and password files](https://docs.pi-hole.net/docker/configuration/)
