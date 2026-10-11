# Home Assistant on clamps

Status: planned, 2026-10-11. No Home Assistant deployment or acceptance is
implemented. This is an optional addition, not a Chef parity requirement or an
additional gate for retiring rupik.

Prerequisite: respect the remaining gates in the
[refactoring roadmap](SKILLET-REFACTOR.md). Implement one slice at a time and
validate it before the next. Follow `../AGENTS.md` and `../skillet/AGENTS.md`.
Read the [Home Assistant decision](../design/home-assistant.md),
[storage](../design/storage.md), [Syncthing](../design/syncthing.md),
[btrbk](../design/btrbk.md), [private UI access](../design/private-ui-access.md),
[secrets](../design/secrets.md), [monitoring](../design/monitoring.md),
and [VM lifecycle](../design/smoke-vms.md).

## 1. Optional container and persistent state

- Add a reusable Home Assistant service module using the existing resource and
  Podman APIs. Declare it in the canonical clamps host profile; UIs, monitoring
  and credentials must consume that same selection. Other profiles stay opted
  out unless their caller explicitly selects it.
- Pin a current official `ghcr.io/home-assistant/home-assistant` release and
  record its tested digest. Use rootful `Network=host`, restart-on-failure/boot,
  and a 60-second container stop timeout with sufficient systemd stop time.
  Disable unattended registry auto-update initially; application upgrades need
  a completed backup and a tested restore path because database migrations can
  prevent rollback by merely reverting the image.
- Require the shared data mount before preparation and startup. Prepare service
  storage under `/var/lib/data/homeassistant`; bind its `config` directory to
  `/config`. Use typed ownership without recursively rewriting existing files.
  Routine apply preserves Home Assistant-managed accounts, integrations,
  automations, database and internal `.storage` state.
- Model optional devices explicitly, using stable serial identities where
  available. Add D-Bus/Bluetooth permissions only when selected. Validate the
  minimum working capabilities and SELinux policy; do not disable host SELinux
  or assume the upstream privileged example is a required global policy.
- Add module-owned Datadog process/health declarations when the monitoring
  capability is selected. Never require a Datadog key solely to run this service.

Exit: routine tests cover selection/omission, rendering, missing/wrong mounts,
ownership, retained state, stop timeout and repeat apply. A named disposable VM
starts an empty instance and preserves synthetic state across apply and reboot
with SELinux enforcing. Real radio discovery remains a physical check.

## 2. Generic private UI access

- Extend the shared UI upstream model to distinguish bridge service DNS from
  a host listener. Preserve existing Pi-hole/Syncthing payloads through an
  explicit compatibility path if the credential schema changes. Keep host,
  environment, instance and SSH target separate.
- Declare `homeassistant` and derive its hostname/aliases through the existing
  generic machinery. Reuse the selected environment's zone, UI prefix, token
  issuance, A/AAAA/CNAME reconciliation and owned disposal.
- Keep Caddy on its bridge. Implement and verify a host gateway route, such as
  `host.containers.internal:8123`; refuse a missing/unusable gateway. Configure
  Home Assistant's forwarded-header trust for the actual proxy source and
  restrict raw listener access so LAN or tailnet clients cannot bypass Caddy.
  Network discovery must still operate independently of HTTP listener policy.
- Inspect the pinned release's supported initial HTTP/proxy configuration.
  Current documentation describes UI-managed settings since 2026.8; do not
  repeatedly overwrite `.storage` or assume a legacy YAML block remains the
  convergence interface. Document any required one-time onboarding step.
- Preserve Home Assistant authentication. Do not add OAuth proxy, Tailscale
  split DNS or Pi-hole UI-zone configuration.

Exit: CI covers host upstream validation, existing bridge routes, aliases,
payload migration and private-value redaction without provider credentials.
Live test-environment acceptance verifies HTTPS onboarding/login, WebSocket
traffic, declared DNS ownership, and raw-port denial from unauthorized sources.
A request from outside the tailnet is denied. Use the existing bounded lifecycle
and record any unavailable source/network checks as pending.

## 3. Scheduled backups and safe publication

- Use Home Assistant native automatic backups, initially daily with seven local
  copies and encryption explicitly enabled. Inspect/configure the supported
  backup interface for the pinned release; document any one-time UI setup.
  Container archives are under `/config/backups`, corresponding to
  `/var/lib/data/homeassistant/config/backups` on the host.
- Add a bounded Rust export workflow using existing effect boundaries. Establish
  successful completion through the supported backup state/API, not file age,
  a guessed sleep, or an archive filename. Read original encrypted archives;
  the ordinary UI download decrypts backups and is unsuitable for this export.
- Copy a completed archive to an ignored temporary file in the destination
  filesystem, verify bytes, then rename atomically into
  `/var/lib/data/syncthing/backups/homeassistant`. Interrupted work must be
  safely retryable, duplicate exports must be no-ops, and malformed/incomplete
  archives must never be published. Do not embed shell programs in Rust.
- Create the destination as an ordinary directory in the existing Syncthing
  leaf. Configure it as a shared folder with an approved receiving peer; do not
  assume that a directory under the subvolume is automatically replicated.
  Reuse mapped Syncthing identity and typed ownership/ACL helpers. Give the
  exporter read access to source archives and Syncthing read access to published
  ones without broad access to the application tree.
- Keep btrbk's clamps selection at `syncthing`. Never snapshot the entire data
  mount or add nested subvolumes. Define receiving-side history/retention
  explicitly so local pruning or synchronization cannot erase every recovery
  copy. Export pruning must never delete the last verified recovery copy.
- Retain the native encryption key/emergency kit in KeePassXC and independently
  back up the encrypted vault. If exporting requires an application API token,
  use supported least-privilege access and existing encrypted delivery, never
  command-line credentials. When implementing key/token requirements, update
  shared secret-audit metadata and regenerate the root secrets checklist;
  distinguish operator-created recovery material from generated test secrets.
- Expose export/backup success and stale-backup failure through module-owned
  monitoring, respecting existing test/lab alert exclusions.

Exit: CI covers completed-only selection, atomic publication, interruption/retry,
identical re-export, corrupt input, missing mount, permissions and retention
using synthetic data. Live acceptance produces an encrypted archive, exports it,
proves a real peer received matching bytes, and includes it in the existing
btrbk snapshot. An unavailable peer is recorded as an open check, not success.

## 4. Isolated recovery acceptance

- Create a separately owned, named disposable VM/instance. Use generated test
  accounts/integrations and a private test encryption key, never the production
  automation identity. Establish persistent network isolation before importing
  sensitive state; do not rely on NAT to block outbound device/cloud access.
- Restore a replicated archive into a fresh compatible Container installation
  through its supported restore interface. Keep restored test state isolated,
  including after reboot. Prove settings, synthetic automation and a known
  database marker survive restoration, routine apply and reboot.
- Demonstrate wrong/missing-key refusal and preservation of the source archive.
  Restore the archive from a disposable copy of the Syncthing btrbk snapshot
  separately to prove historical recovery, without modifying the live leaf.
- Dispose only owned resources through the existing Rust lifecycle, including
  test DNS/token/device cleanup. Never export backups or keys into public logs.

Exit: both replicated-archive and snapshot recovery work; original backup and
independent recovery key remain usable. No restored instance contacts live
home devices or production cloud integrations. Record aggregate evidence and
limitations in `../butane/ACCEPTANCE.md`.

## 5. Production activation and documentation

- Confirm the actual device/integration inventory before requesting radios,
  Bluetooth permissions or companion services. MQTT/Zigbee2MQTT and similar
  separate containers need their own state/backup plans; Home Assistant's
  Container backup does not back them up automatically.
- Configure production UI/credentials through the existing vault workflow;
  validate real-client DNS, HTTPS, login and device discovery on clamps.
  Keep smoke names, identities, archives and destinations separate from prod.
- Confirm daily encrypted backup, last-success monitoring, off-machine receipt,
  recovery-key availability and free-space/retention policy. Keep an independent
  recovery copy before an application update and test restore compatibility.
- Update designs and operational README instructions, including onboarding,
  private URL, backup export, key storage, restore and upgrade rollback.
  Use placeholders in the public repo; never commit real domains, identifiers,
  archives or credentials. Install nothing on the workstation.

Exit: run all applicable local CI checks and named-VM application acceptance;
record unavailable hardware/provider checks honestly. Make focused commits after
validation; do not push unless authorized. Update each slice's implemented vs
pending status, and keep physical discovery/production acceptance distinct from
VM acceptance.
