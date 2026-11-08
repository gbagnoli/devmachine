# Home Assistant

Rootful Home Assistant Container, selected explicitly by the host profile.
Clamps selects it with host Bluetooth access; joey does not. No MQTT or USB
radio passthrough is selected. Container mode has no Supervisor/app store.

## Apply and onboard

From `ublue/skillet`, use normal `cargo run --release -p skillet -- apply clamps`.
Home Assistant follows the stable image channel with registry auto-update enabled.
Open the declared `homeassistant.<host>.<ui-domain>` HTTPS endpoint from an allowed
user-owned tailnet client and complete native onboarding, creating the shared
owner account with a recovery password. Restart `homeassistant.service` once after
onboarding. Subsequent authorized tailnet visits use that shared account without
a second password prompt. Everyone allowed by the UI policy has the owner's HA
permissions; use this only for your approved household users.

The module attaches `auth_providers: !include skillet-auth.yaml` under the root
`homeassistant:` section, preserving a one-time `.skillet-before-shared-auth.yaml`
copy. If that section already exists without the managed include, explicitly add
the include there before apply; existing authentication is never overwritten.
The managed include follows the active owner and retains native password login
for loopback recovery. Ordinary account/app settings stay application-owned.
Native login tokens do not bypass Caddy's per-request authorization. Host port 8123 binds loopback only; Caddy uses a confined Unix relay.
The host listener policy also denies non-loopback TCP 8123 in both families.
Do not change the application's HTTP bind/proxy settings to wildcard interfaces.

State: `/var/lib/data/homeassistant/config` → `/config`. Existing configuration
is preserved on apply, including accounts, automations, integrations and recorder.
An offline application-image generator publishes missing configuration and includes.
It initializes confirmed loopback/proxy settings through the application
HTTP store API, avoiding an unconfirmed migration trial. Existing application
configuration and HTTP state are preserved.

## Bluetooth on clamps

The clamps image includes BlueZ and `xdg-dbus-proxy` and enables
`bluetooth.service`. The selected profile grants `NET_ADMIN`/`NET_RAW` and mounts
`/run/skillet-ha-bluetooth` read-only at `/run/dbus`. The confined
`skillet-ha-bluetooth.service` filters bus traffic to `org.bluez`; the host system
bus is not mounted into HA. Its prerequisite policy unit loads the dedicated
SELinux domain before startup. Apply Skillet to recreate HA with this mount.

Check `systemctl status bluetooth.service skillet-ha-bluetooth.service` and
`journalctl -u skillet-ha-bluetooth.service -u homeassistant.service`. Bluetooth
devices and pairing remain application-managed. Do not relabel `/run/dbus` or
disable SELinux to work around a connection error.

For enforcing-SELinux VM acceptance, upload `tests/bluetooth.sh`,
`tests/bluetooth.Containerfile`, `src/skillet-ha-bluetooth{.cil,.service,-policy.service}`,
and `src/wait-ready.sh`
to one private guest directory through the owned VM adapter, then run the shell
helper as root through that adapter. It refuses physical hosts, checks allowed
BlueZ calls and denied systemd/direct-bus calls from a real container, and repeats
after broker restart. It builds a disposable probe image; no vault or radio is
required. Reboot through `skillet test vm reboot HOST INSTANCE` and rerun the
helper with `--verify`; it first checks boot startup without reinstalling the
policy or repairing units. Physical adapter discovery remains a separate
acceptance check.

## Enable encrypted backups

1. In the Home Assistant administrator profile, Security, create a dedicated
   long-lived access token for Skillet. Native backup APIs require administrator
   access; this token is not a browser password.
2. Store it in KeePassXC, **Password** field:
   `skillet/environments/prod/hosts/<host>/homeassistant/api-token`.
3. Unlock the vault, run `skillet secrets check --host clamps`, then normal apply.
   Skillet generates and saves `.../homeassistant/backup-password` before delivery.
   Keep an independent vault backup; do not delete this key while retaining archives.
   If native automatic backups were already configured, store their existing
   encryption password at that path before applying; Skillet will not replace it.
4. Run `skillet secrets deliver clamps homeassistant --activate` (with normal SSH
   target overrides if needed) to request the first immediate backup/export.
   Subsequent exports run daily at 04:00 Europe/Paris, with catch-up after a missed
   run. Native encrypted backups run daily with seven local copies; their initial
   creation time is 03:00. Missing API-token entry skips delivery during normal apply.
5. Add a Syncthing folder at `/var/syncthing/data/backups/homeassistant` (the
   container path), shared with an approved receiving peer. Verify received bytes and
   independent receiving-side history/retention. Directory placement alone does
   not establish replication.

Local original archives: `/var/lib/data/homeassistant/config/backups`.
Published encrypted archives: `/var/lib/data/syncthing/data/backups/homeassistant`.
The exporter requires completed local backup metadata, validates the encryption
password through Home Assistant, checks the archive and atomically publishes a
verified byte copy. It never exports the decrypted UI download or live database.
Repeated publication is a no-op; conflicting destination bytes cause an error.
After every successful export pass, Skillet keeps the seven newest managed
`ha-<id>.tar` archives by the backup date inside each archive. It refuses
pruning if a managed archive is malformed or changed during inspection; other
files are left alone. Native retention also keeps seven local copies. Btrbk
snapshots the Syncthing subvolume, but a separate receiving copy and history
are still needed for disk loss.

`skillet-homeassistant-backup.timer` activates the credential-gated exporter.
Inspect `journalctl -u skillet-homeassistant-backup.service`. On the guest,
`systemctl start skillet-homeassistant-backup.service` runs an immediate check.
The credential-capable guest command is `skillet homeassistant backup`; do not
put credentials into command arguments or shell environment. Datadog gets the
module's process check and backup freshness collector when selected; absent
onboarding credentials are reported as unconfigured, not fake successful backups.

Do not overwrite existing backup settings/passwords through routine provisioning.
If the native password no longer matches KeePassXC, reconciliation fails for
operator recovery. Preserve the old key for old archives when changing it.

## Test and recover

Use a named disposable clamps VM through the [VM guide](../vm/README.md).
Allow at least 64 GiB virtual disk capacity for application acceptance; the stock
FCOS QEMU source is only 10 GiB. The owned guest assertion helper can grow an
existing plain-root VM; expand its `vda4` partition and Btrfs data mount before
provisioning applications. Do not use these virtual disk names on hardware.

Update the VM binaries with `skillet test vm update HOST INSTANCE` first.
Build `cargo build --release -p skillet --example homeassistant_acceptance`, then
upload the Cargo-reported artifact with the VM `guest_command` example and run it
with guest sudo. It refuses non-disposable identities and generates a synthetic
native account/key. It checks HTTPS/native API/WebSocket upgrade, backup/export,
wrong-key refusal and repeat apply; run it again with `--verify` after reboot.
`tests/replication.py` provides guest assertions for an explicitly linked disposable
Syncthing peer and matching btrbk snapshots. Run each assertion through that same
owned VM adapter, never against a physical host. These fixtures do not replace
allowed-user tailnet or phone acceptance.
`tests/shared_auth_acceptance.py` runs inside the disposable HA container after
synthetic onboarding: it checks native passwordless flows for forwarded IPv4/IPv6,
the same shared account, API/WebSocket tokens and local password fallback. Run it
through the owned guest adapter; the forwarded addresses are local test inputs,
not proof that an allowed desktop passed Caddy's real identity gate.
For VM-to-workstation replication acceptance, run
`python3 crates/homeassistant/tests/acceptance_peer.py ABSOLUTE_GUEST_HELPER ABSOLUTE_BUTANE HOST INSTANCE`.
It uses an existing cached Syncthing image and free loopback ports 22100/28384,
links a temporary receiver, checks encrypted bytes and a btrbk snapshot, then
removes the peer declaration/container. Override those ports with `--sync-port`
and `--gui-port` if occupied. No vault or production Syncthing peer is used.

Routine CI needs no vault/provider access. Use generated test accounts/keys and
never restore production automation into a guest with device/cloud connectivity.
The [plan](../../../plan/HOME-ASSISTANT.md) tracks runtime/peer/restore acceptance.
Restore through Home Assistant's supported Container restore workflow, using a
compatible image and retained encryption key. Keep an independent archive before
upgrades; reverting the container image may not reverse database migrations.

For a live restore on an existing Skillet host, copy
[`scripts/live-restore.sh`](scripts/live-restore.sh) and
[`scripts/restore-stage.py`](scripts/restore-stage.py) to the same directory on
that host, then run the shell script as root with the absolute path of a
completed encrypted archive. It first restores offline with networking disabled,
then stops Home Assistant, moves its current config aside, restores the archive,
checks the owner/UI/exporter and retains the previous config for rollback. If
verification fails, it returns the previous config to the live path. The archive
and recovery password remain separate; the script reads the host's encrypted
Skillet credential and does not take a password argument.

See [deployment/backup design](../../../design/home-assistant.md).
