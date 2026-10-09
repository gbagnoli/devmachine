# UniFi container on clamps

Prerequisite: complete [the Skillet refactoring roadmap](SKILLET-REFACTOR.md)
before proceeding with the remaining feature milestones in this plan.
Existing acceptance evidence and deferred tasks remain applicable.

Status: implementation and empty-controller acceptance passed on the
disposable x86_64 VM on 2026-10-03. UI backup import and restored-data persistence passed on 2026-10-09.
Authenticated UI verification, final disposable cleanup and production migration
remain pending.

## Current behavior

Chef recipe: `site-cookbooks/rupik/recipes/unifi.rb`.

- Rootful Podman, `Network=host`, image `docker.io/jacobalberty/unifi:latest`,
  `User=unifi`, `TZ=Europe/Madrid`, `Restart=always`, network-online ordering,
  and registry auto-update.
- Rupik is ARM64; the running image package is
  `10.0.162-32076-1`. Clamps is x86_64.
- `/srv/unifi` is bind-mounted to `/unifi`, currently 1.2 GiB on Btrfs. Data
  files are owned by 999:999. The image user is UID/GID 999; Chef's host
  account `unifi` UID/GID 2666 is not the data owner. `/unifi/run` is a
  separate generated Podman volume.
- Active listeners: TCP 8080, 8443, 6789, 8843, 8880; UDP 3478, 10001. Local
  HTTPS on 8443 returned 302. The Chef repo has no UniFi nginx route.
- Eight small `.unf` backups were listed through October 2026. Restore validity
  and off-host availability are unverified.

## Scope and implementation

Implemented: reusable `skillet-unifi` module, called by clamps full apply. It
uses rootful Podman with mandatory host networking for device discovery and
adoption, `/var/lib/data/unifi` mounted at `/unifi`, the Chef image family,
`Europe/Madrid`, restart-at-boot, and registry auto-update. It requires the
shared data mount and prepares the service subvolume. The image's `unifi` user
owns the application state as UID/GID 999; Skillet sets those numeric IDs on
only the subvolume root and preserves existing application-owned files. The
host does not need matching account names. Rupik UID 999 is
`systemd-coredump`, not Chef's separate UID 2666 `unifi` account.

Do not add a Caddy route or a new credential flow for UniFi. UniFi Site Manager
at `unifi.ui.com` provides remote access to a locally running controller. The
control plane, configuration, and device management remain on the server; local
management remains available on the host network. Remote access must be enabled
for the controller's UI account.

## Migration and acceptance

1. **Passed:** Test the Quadlet/module on a named disposable x86_64 VM with an
   empty controller: service starts, remains active, uses intended ownership,
   has `Network=host` for discovery/adoption, and survives repeated apply.
   See `butane/ACCEPTANCE.md`. Reboot survival, expected host listeners, and
   confirmation that no Caddy/Cloudflare setup is needed remain to verify.
2. Restore one private rupik `.unf` Network backup in a dedicated x86_64 VM
   using a compatible application version. Complete the isolated acceptance
   below. Site Manager access is a separate cutover check: the restored copy
   must not contact devices or the cloud while rupik remains the live controller.
3. Before cutover, restore the selected backup on clamps, verify remote access
   and device connectivity, and retain rupik plus an independent backup for
   rollback. Do not run two controllers against the same devices concurrently.

Routine tests already cover host-network Quadlet rendering, numeric data-root
ownership, mount refusal, repeated apply, and preservation of existing files;
the canonical profile selects UniFi for Clamps. Empty-controller runtime
acceptance passed on 2026-10-03. Backup restore and production/device acceptance
remain pending. Record new commands and results in `butane/ACCEPTANCE.md`.
Device migration stays unverified until a backup restore or deliberate
re-adoption succeeds.

Read `skillet/AGENTS.md`, `design/storage.md`, `design/secrets.md`, and
`design/smoke-vms.md` before implementation.

References: [UniFi cloud architecture](https://help.ui.com/hc/en-us/articles/30319146145175-Understanding-UniFi-Cloud-Architecture),
[remote access](https://help.ui.com/hc/en-us/articles/360012192813-Introduction-to-UniFi),
[local management](https://help.ui.com/hc/en-us/articles/28457353760919-UniFi-Local-Management).

## Isolated backup-restore acceptance

This work is in progress. UI import, repeat module apply, restored configuration
hashes and reboot persistence passed on 2026-10-09. Authenticated UI inspection
and final disposal remain pending. On 2026-10-08 the workstation reached rupik but SSH
failed with `Permission denied (publickey)`. The user supplied a private October
1 Network backup (212,880 bytes); its original automatic-backup filename
identifies application version 10.0.162. Current source runtime access remains
unverified. Private `.unf` files are ignored by the Skillet checkout.

1. Obtain the Network application backup through its Backups UI or read-only
   source access. Keep it outside the checkout in a mode-0700 directory, with
   mode-0600 files. Record its date, byte count and source application version
   privately; never commit the backup, restored configuration, identities or
   credentials. Retain an independent copy for eventual rollback.
2. Create a dedicated named disposable VM using the existing Rust lifecycle.
   Keep the existing smoke VM untouched. Record the target application version
   and image digest; the target must support the selected backup. Pull the
   image before fencing network access. Do not copy the ARM MongoDB data tree.
3. Stop UniFi before importing anything. Install a **VM-only, persistent**
   isolation policy ordered before UniFi starts: permit guest loopback,
   the owned SSH management connection, DHCP and IPv6 neighbour discovery
   needed to retain the management link after reboot; reject other IPv4/IPv6
   input and output. Disable automatic image updates during this test. NAT alone is
   insufficient: it still permits outbound access to production networks and
   cloud services. Do not enroll this restored controller for cloud access.
   Before first activation on the empty guest, arm a short automatic rollback
   timer. Prove fresh SSH connectivity, cancel the timer, then recheck the fence
   before starting the controller or uploading the backup. Never arm a timer
   that can open networking once production state is present.
   Validate allowed SSH/local HTTPS and denied LAN, tailnet and public egress
   before import and after reboot. An unavailable probe target does not prove
   isolation; inspect the rules and their deny counters as well.
4. Inspect the installed application's supported restore interface. Prefer
   its documented command if available; otherwise use its `.unf` restore UI. If authenticated UI
   interaction is required, use a local browser through an ownership-checked
   tunnel; add that capability to the shared VM transport if needed, never
   bypass the lifecycle with a separately assembled SSH command. Do not guess
   an undocumented restore API or transmit account passwords in CLI arguments.
   The isolated driver is `cargo build --release -p skillet_unifi --example
   acceptance_apply`: deploy its reported executable through the owned VM
   transport and run it as root only on a prepared disposable guest. It invokes
   the production UniFi recipe alone, without importing state or needing
   credentials for unrelated services. Record module-only acceptance separately
   from full-host acceptance.
5. Verify locally that restoration completes and the expected sites, networks,
   WLANs and device inventory are present. Record only aggregate counts and
   pass/fail evidence publicly. Devices staying offline is expected in this
   isolated test. Verify rootful host networking, UID/GID 999 data ownership
   and local management HTTPS. No Caddy/Cloudflare provisioning is required.
6. Repeat the production UniFi module apply with the isolated driver and
   confirm the restored state remains intact. Record full-host acceptance
   separately; the isolated driver does not configure the other services.
   Reboot using the owned lifecycle, prove the fence remains effective before
   UniFi starts, and confirm HTTPS and restored state survive. Ordinary apply
   must never perform the import. Capture private failure diagnostics.
7. Stop the restored controller and destroy only the owned disposable VM.
   Verify cleanup and retain the independent private backup. Record evidence
   in `butane/ACCEPTANCE.md`; mark restore as passed only after these checks.

Production adoption, inform-address changes, device forgetting/resetting and
Site Manager registration are excluded from the isolated test. Verify them
only during an explicitly scheduled cutover with the old controller stopped
and rollback available.

Restore reference: [Ubiquiti Network backups and migration](https://help.ui.com/hc/en-us/articles/360008976393-Backups-and-Migration-in-UniFi).

### Current live handoff — 2026-10-09

- Retained VM: `clamps` / `unifi-restore`, native `qemu:///session`, SSH 2202.
  UniFi version 10.0.162-32076-1, rootful host networking, data-root 999:999/0750.
- Operator uploaded the backup through the setup UI. Database aggregates:
  two sites, one network, four WLANs, three devices, one admin. Source inventory
  equivalence remains unverified. CLI restore attempts failed previously;
  those failures did not establish backup corruption.
- Repeat production module-only apply before and after owned reboot passed;
  network/WLAN configuration hashes stayed identical. HTTPS returned 200 after
  boot startup, service active with zero restarts. This is not full-host acceptance.
- Persistent isolation survived reboot and is required before UniFi activation.
  Public IPv4 HTTPS remains blocked; separate LAN/tailnet/IPv6 probes remain
  unverified. No provider credentials or cloud/device access were enabled.
- Login with the operator’s recorded password failed; an older password is
  possible. SMTP recovery is unavailable. Retain the isolated VM for credential
  investigation; do not alter database credentials or remove the fence.
- Remaining: operator authenticated UI/configuration inspection, then stop and
  dispose through the owned lifecycle. Preserve independent private backups.
  Production adoption and Site Manager remain scheduled cutover checks.
- Email delivery is tracked in [the follow-up plan](CLAMPS-EMAIL.md).
