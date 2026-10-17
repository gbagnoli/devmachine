# Home Assistant

Rootful Home Assistant Container, selected explicitly by the host profile.
Clamps selects it; joey does not. No MQTT, radios or Bluetooth permissions are
selected. Container mode has no Supervisor/app store.

## Apply and onboard

From `ublue/skillet`, use normal `cargo run --release -p skillet -- apply clamps`.
Home Assistant's version is pinned in this crate; registry auto-update is disabled.
Open the declared `homeassistant.<host>.<ui-domain>` HTTPS endpoint from an allowed
user-owned tailnet client and complete native onboarding. Native login remains
required. Host port 8123 binds loopback only; Caddy uses a confined Unix relay.
The host listener policy also denies non-loopback TCP 8123 in both families.
Do not change the application's HTTP bind/proxy settings to wildcard interfaces.

State: `/var/lib/data/homeassistant/config` → `/config`. Existing configuration
is preserved on apply, including accounts, automations, integrations and recorder.
An offline pinned-image generator publishes missing configuration and includes.
It initializes confirmed loopback/proxy settings through the pinned application
HTTP store API, avoiding an unconfirmed migration trial. Existing application
configuration and HTTP state are preserved.

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
   Subsequent timer checks run hourly; native encrypted backups run daily with
   seven local copies. Missing API-token entry skips delivery during normal apply.
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
Published copies are retained without automatic deletion initially; monitor space
and configure independent receiving retention before pruning. Native retention
still limits the application's local copies to seven.

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

See [deployment/backup design](../../../design/home-assistant.md).
