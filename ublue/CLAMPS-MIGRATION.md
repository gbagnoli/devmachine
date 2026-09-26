# Clamps migration draft

Status: implementation checkpoint, 2026-09-26. Milestones 1 and 2 were delegated and implemented; final VM acceptance remains outstanding. Later milestones remain a planning draft.

Detailed delegation drafts: [agent 1 — Skillet convergence](CLAMPS-AGENT-1.md) and [agent 2 — reproducible VM boot](CLAMPS-AGENT-2.md). Agent 1 hands off a tested base-phase interface and smoke check; agent 2 supplies final uCore runtime evidence. Both require parent review before acceptance.

## Goal and starting point

Provision clamps with uCore + Skillet and migrate the required behavior and persistent state of the Chef-managed rupik. Complete one milestone at a time; add reusable Skillet resources when a service needs them.

Repositories refreshed with `git pull --ff-only`:

- devmachine, branch `skillet`, commit `1fa4fd4`: already current.
- ucore-images, branch `main`, commit `759f195`: now includes serialized common/host builds, `wol` and `et`, and removal of the `/usr/local/bin/wol` wrapper. CI status and hardware operation were not checked.

Skillet currently has hardening, container/secret primitives, and partial Pi-hole configuration. Its broader OS-hardening function is a placeholder. Butane defines installation and autorebase services, but binary delivery, credential provisioning, resolver continuity, and boot sequencing are incomplete.

The starting-point description above refers to the pre-migration commits. The current checkpoint adds explicit base-phase apply, service activation recovery, a real-runtime smoke runner, staged binary delivery, resolver configuration and deployment-aware bootstrap. Static/unit validation does not establish successful installation: see [VM acceptance evidence](butane/ACCEPTANCE.md) for the remaining runtime checks.

The next acceptance gate is a fresh disposable VM reaching the signed deployment and completing base apply, followed by the smoke runner and reboot/repeatability checks. Resolve that before implementing storage. The selected storage direction is boxy's single Btrfs filesystem with separate OS/data subvolumes; the mount path and final physical disk remain open in the [milestone 3 discussion](CLAMPS-STORAGE-CREDENTIALS.md).

## Boundaries

- Image: packages, static OS files, baseline unit enablement/masking. Preserve common-before-host image builds.
- Ignition: installation layout, initial access, bootstrap configuration and credential delivery.
- Skillet: continuing convergence of host values, application configuration, secrets, storage resources and containers.
- Public repositories and images must exclude secret values, including secrets compiled into host binaries. Decide which topology values also require private delivery.
- Keep the production Samsung SSD and functioning DNS on rupik until cutover. Test on the stock clamps SSD or disposable VM disks.
- Leave desktop `ublue/tailscale/` and `ublue/llama/` outside this work.
- Preserve Rust safety/error-handling rules and idempotency. Replace the blanket `systemctl --wait` rule with waiting for startup job completion: `--wait` waits for service termination and can hang on persistent services.

## Milestone 1: make Skillet convergence reliable

Scope: repair the existing execution path before adding another application.

- Correct service start/restart waiting and propagate failures.
- Let Quadlet's generator process `[Install]`; remove explicit enabling of generated services.
- Ensure an unchanged desired service is started if inactive. Restart/recreate it when its configuration or consumed secret changes.
- Handle interrupted applies: a file written before a failed reload/start must not prevent recovery on the next apply.
- Avoid configuration churn, including competing ownership updates to the same directories.
- Add focused unit tests for these failure and convergence cases, using the existing separate test modules.
- Replace the recording-comparison acceptance check with a small disposable VM smoke scenario using real systemd, Podman and a harmless container. Check first apply, second apply, stopped-service recovery, changed configuration, and reboot persistence.
- Keep recording only if useful for diagnostics. Update `skillet/AGENTS.md` and documentation to reflect the replacement; do not maintain a second harness merely to preserve old recordings.

Done when: apply completes, failures are visible, a second apply causes no unnecessary changes/restarts, and interrupted work can recover. Formatting, workspace tests and pedantic Clippy pass; the real-runtime smoke scenario passes.

## Milestone 2: boot a reproducible clamps base

- Separate VM disk selection from physical installation; remove the physical install's dependence on `/dev/vda`.
- Establish SSH access, users/sudo requirements and hardware/Secure Boot preparation before unattended provisioning.
- Supply working bootstrap DNS in Ignition. The image masks resolved, so DNS must work before Skillet can run or pull containers. Decide the resolver owner and upstreams, then have Skillet maintain that policy.
- Deliver a versioned Skillet artifact to a location compatible with ostree and rebases. Decide image delivery versus a persistent runtime location; do not assume an Ignition-written `/usr/bin` binary survives rebases.
- Sequence unsigned rebase, signed rebase and apply using the actually booted deployment. Existing markers record requested rebases, not proof that the target deployment has booted.
- Decouple baseline provisioning from application credentials so the base can converge before Pi-hole is introduced.

Done when: a fresh VM reaches the signed clamps deployment, retains SSH and DNS through reboots, and runs the baseline apply successfully. Repeat on the stock SSD after hardware preparation.

## Milestone 3: storage and credentials for the first application

Resolve only the decisions needed for Pi-hole first.

- Decide the test and final `/srv` layout, mount identity, and Pi-hole persistence path. Chef currently uses `/srv/pihole`; Skillet currently uses `/etc/pihole`.
- Put formatting/partition creation in the installation path. Skillet must not format existing data during convergence.
- Make applications depend on their real data mount so a missing disk cannot silently produce an empty replacement data directory on the OS filesystem.
- Add directory/subvolume/ownership support as required. Defer container graphroot relocation unless chosen explicitly.
- Implement SOPS/age delivery for one credential end to end. Choose individual credentials versus a bundle, safe serialization, and handling of generated Ignition artifacts.
- Define TPM requirements, encryption timing, recovery and rotation. Make encryption and cleanup recoverable after interruption; verify a usable encrypted credential before deleting the input.
- Decide Podman secret storage protection explicitly. Its default file driver does not inherit systemd credential encryption.

Done when: Pi-hole's storage and password can be provisioned, recovered after interruption, and rotated without leaking values into repository files, command arguments or logs. Reboot/rebase recovery works. The persistent-data mount failure case is exercised.

## Milestone 4: working Pi-hole on an isolated test address

- Add the dual-stack network resource, explicit container attachment and auto-update configuration.
- Decide whether to retain the Chef web pod or publish Pi-hole ports independently. Preserve the eventual nginx integration contract.
- Correct Pi-hole password-file consumption, container identity/permissions, SELinux labels, DNS publication, upstreams, web port and custom-record format for the selected image version.
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
| 5 | nginx + OAuth2 proxy + ACME | Domains/routes, access rules, Cloudflare real-IP handling, certificates, renewal and reload |
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

- Base bootstrap: artifact delivery and resolver ownership/upstreams.
- Storage: stock-SSD test layout, final disk arrangement, Pi-hole path, container graphroot.
- Credentials: artifact handling, TPM/recovery policy, Podman storage and rotation.
- Services: actual DNS records, domains/routes, Tailscale enrollment, Nebula Sync, Datadog and legacy networking.
- Cutover: address/identity transitions, outage window and rollback duration.

## Technical references

- [systemctl waiting semantics](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml)
- [Quadlet enablement](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html#enabling-unit-files)
- [Podman secret drivers and replacement](https://docs.podman.io/en/latest/markdown/podman-secret-create.1.html)
- [Pi-hole configuration and password files](https://docs.pi-hole.net/docker/configuration/)
