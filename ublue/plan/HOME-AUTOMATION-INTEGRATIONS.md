# Home automation companions on clamps

Status: planned. Home Assistant Container and its encrypted backup exporter are
implemented; Mosquitto, Zigbee2MQTT and ESPHome Device Builder are not selected
or deployed. No physical radio or sensor has been accepted. Follow
[the Home Assistant plan](HOME-ASSISTANT.md) for existing work and its pending
isolated restore rehearsal. This is an optional clamps capability, separate from
rupik cutover parity.

Read [the deployment decision](../design/home-assistant.md),
[secrets](../design/secrets.md), [private UI access](../design/private-ui-access.md),
[storage](../design/storage.md), [Syncthing](../design/syncthing.md) and
[btrbk](../design/btrbk.md). Keep application-managed integrations in Home
Assistant; Skillet owns service deployment, declared connections, backup export
and monitoring. No real device identifiers, network names or credentials belong
in this public plan.

## 1. Inventory and independent HA recovery

- Record the Zigbee coordinator model, firmware, adapter protocol and stable
  `/dev/serial/by-id/` path before enabling its service. Check whether that
  adapter supports coordinator backup. Record Dyson model(s), Hue bridge and
  Overkiz gateway model/API mode, and the first ESPHome device(s).
- Complete the existing HA production backup setup after onboarding: dedicated
  administrator API token, vault-held encryption password, daily native backups,
  atomic export to the declared Syncthing folder, actual peer receipt and
  independent receiving retention. Run the pending isolated restore rehearsal
  before treating those archives as a recovery path.

Exit: inventory states which integrations can use local APIs and whether Zigbee
radio recovery is supported. An encrypted HA archive can be restored from both
the replicated copy and a btrbk snapshot; no test restore controls real devices.

## 2. Mosquitto broker

- Add a reusable optional service selected explicitly by the clamps host caller.
  Use a pinned official container, a dedicated Podman network shared with
  Zigbee2MQTT, and persistent state under `/var/lib/data/mosquitto` outside
  Syncthing. HA's host-network container reaches a loopback-published broker
  listener; Zigbee2MQTT uses the broker's network name. Open a LAN listener only
  when a declared device needs MQTT directly. Never expose MQTT publicly.
- Disable anonymous access. Give HA, Zigbee2MQTT and any direct device distinct
  identities and scoped topic permissions. Generate or read passwords with the
  KeePassXC use-or-create flow; deliver without plaintext command arguments,
  environment variables, Quadlets or logs. Add required/optional entries to the
  shared secret catalog and regenerate `SECRETS.md` when implemented.
- Configure persistence for retained discovery/state and durable sessions.
  Add module-owned service and connectivity monitoring. Use systemd ordering so
  the broker starts before Zigbee2MQTT; HA remains usable if it is absent.

Exit: in a named disposable VM, authenticated HA and Zigbee2MQTT clients can
publish/subscribe as allowed, anonymous and out-of-scope topics are rejected,
and retained messages survive restart/reboot. Apply preserves existing data.

## 3. Zigbee2MQTT and radio boundary

- Add a reusable optional Quadlet with `/app/data` under
  `/var/lib/data/zigbee2mqtt`. Pin a release; map only the selected serial device
  and required udev information. Keep SELinux enforcing and validate actual
  device access. A missing radio must leave an explicit unconfigured/deferred
  service state, without making the rest of host apply fail.
- Connect to Mosquitto with its own credential, enable HA MQTT discovery, and
  declare a private UI through the existing Caddy/tailnet identity policy. Keep
  the raw frontend unreachable from LAN and unauthorized tailnet clients.
  Preserve Zigbee2MQTT-managed device names, groups, network keys and pairing
  state across applies; never regenerate a populated network.
- Confirm coordinator backup support for the chosen adapter before pairing.
  Include the supported coordinator backup and the full `data` directory in the
  companion archive. If unsupported, record that replacing the adapter may
  require re-pairing before declaring recovery complete.

Exit: a VM fixture exercises broker discovery, UI/WebSocket and data retention;
physical acceptance confirms serial access, pairing and operation after reboot.
Radio replacement remains a separate hardware recovery check.

## 4. HA integrations and ESPHome

- Configure HA's built-in Hue, Overkiz, MQTT and ESPHome integrations in HA.
  Prefer Overkiz local API when the actual gateway supports the needed devices;
  retain cloud mode if local mode omits required entities. Pair Hue to its
  bridge locally. Keep any vendor tokens/account recovery material in KeePassXC;
  HA's integration state is covered by its native archive.
- For Dyson Local, inventory the device model and pin a reviewed release of a
  maintained custom integration compatible with the pinned HA version. It is
  application code in HA `/config`, not another service container. Test device
  control and a backup/restore with the integration installed.
- Select an optional ESPHome Device Builder container when sensors are ready.
  Persist `/config` under `/var/lib/data/esphome`, pin its release, and use host
  networking only if mDNS/OTA discovery requires it. Expose its dashboard only
  through the private Caddy identity path and isolate its raw port. Do not grant
  USB or privileged access until a specific flashing workflow requires it.
  Use the native ESPHome API with per-device encryption keys. Store firmware
  YAML, `secrets.yaml` and recovery keys in the companion backup; do not publish
  them as clear Syncthing files.

Exit: each selected real device appears in HA, survives restart, and can be
controlled from an allowed tailnet client. Unauthorized UI and raw-port access
are denied; cloud-dependent devices are identified in the acceptance record.

## 5. Companion backup, restore and monitoring

- Add a bounded, versioned companion backup for Mosquitto, Zigbee2MQTT and
  ESPHome. Quiesce the writers or use their supported backup interface, and
  include broker persistence, all Zigbee2MQTT data/coordinator backup, and
  ESPHome source YAML/secrets. Exclude rebuildable caches and logs. Capture
  service versions and checksums in a manifest. Never treat a live directory
  copy as a consistent backup.
- Encrypt the archive with an `age` recipient before publishing it atomically
  under the caller-declared Syncthing files root, in `backups/home-automation`.
  Keep the decryption identity independently backed up in KeePassXC; only the
  recipient is delivered to the host. Reuse the existing `syncthing` btrbk leaf.
  Define receiving-side retention before enabling pruning. Preserve completed
  archives on retry or interruption, and alert on missing/stale exports and
  missing peer receipt without paging for disposable `lab` machines.
- Rehearse recovery in an isolated named VM: restore HA's encrypted native
  archive and the companion archive, start broker then Zigbee2MQTT then HA,
  verify MQTT discovery and ESPHome configuration, and repeat from a btrbk copy.
  Use synthetic Zigbee data/radio emulation in CI; real coordinator restoration
  requires the selected hardware. Refuse wrong keys and mismatched archive
  manifests without altering the source archives.

Exit: CI checks archive scope, encryption, atomic publication and retry; a
disposable restore proves service state survives reboot and reapply. Physical
acceptance proves real device reconnection and coordinator recovery where
supported. Record evidence in [acceptance](archive/ACCEPTANCE.md); do not mark
VM checks as hardware acceptance.

## Upstream references

- [Home Assistant MQTT](https://www.home-assistant.io/integrations/mqtt/),
  [Hue](https://www.home-assistant.io/integrations/hue),
  [Overkiz](https://www.home-assistant.io/integrations/overkiz/),
  [ESPHome](https://www.home-assistant.io/integrations/esphome)
- [Mosquitto authentication](https://www.mosquitto.org/documentation/authentication-methods/)
  and [persistence](https://www.mosquitto.org/man/mosquitto-conf-5.html)
- [Zigbee2MQTT container](https://www.zigbee2mqtt.io/guide/installation/02_docker.html),
  [HA discovery](https://www.zigbee2mqtt.io/guide/usage/integrations/home_assistant.html),
  [backup API](https://www.zigbee2mqtt.io/guide/usage/mqtt_topics_and_messages.html),
  [adapter backup support](https://www.zigbee2mqtt.io/guide/adapters/)
- [ESPHome container](https://esphome.io/install/docker/),
  [secret handling](https://esphome.io/guides/security_best_practices/)
- [Maintained Dyson integration](https://github.com/libdyson-wg/ha-dyson)
