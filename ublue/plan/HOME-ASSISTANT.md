# Home Assistant on clamps

Status: container, private host-loopback HTTPS transport, native encrypted
backup/export and optional credential delivery are implemented. First disposable
runtime acceptance passed 2026-10-16. Physical clamps activation, owner setup,
encrypted export, offline archive extraction and a live restore passed on
2026-10-17; remaining checks are listed below and in
[the evidence record](archive/ACCEPTANCE.md). No Home Assistant devices, MQTT
or companion services are selected yet. Their
deployment and recovery are tracked in the
[home automation companions plan](HOME-AUTOMATION-INTEGRATIONS.md). This optional
addition is not a Chef parity requirement or a gate for retiring rupik.

Preserve the separately parked gates in the
[refactoring roadmap](SKILLET-REFACTOR.md); they do not block independent slices.
Preserve the implemented boundaries and complete remaining acceptance scopes. Follow `../AGENTS.md` and `../skillet/AGENTS.md`.
Read the [Home Assistant decision](../design/home-assistant.md),
[storage](../design/storage.md), [Syncthing](../design/syncthing.md),
[btrbk](../design/btrbk.md), [private UI access](../design/private-ui-access.md),
[secrets](../design/secrets.md), [monitoring](../design/monitoring.md),
and [VM lifecycle](../design/smoke-vms.md).

## Current slice status

| Slice | Implementation | Acceptance still required |
| --- | --- | --- |
| 1. Container | Implemented; clamps opts in, other callers do not; repeat apply and reboot accepted in the disposable VM; running on physical clamps | Device-specific capability acceptance awaits real devices |
| 2. Private HTTPS | Implemented; real local TLS/native API/WebSocket checks pass | Provider staging ACME/DNS and explicit source/identity denial pass; allowed-user browser/phone/companion and separate-network denial remain live checks |
| 3. Backups | Implemented; native encryption/export, independent rootless receiver and btrbk inclusion pass in disposable acceptance; production token, key and first encrypted export now pass | Production off-machine receipt and independent receiving retention |
| 4. Restore | Offline extraction and live clamps restore from the exported archive passed; a reusable host procedure is saved | Isolated full restore from a received peer and snapshot copy in a disposable VM, with synthetic state and reboot/reapply |
| 5. Hardware | HA runs on clamps with no device permissions selected | No devices are available; MQTT/radios remain unselected |

## Short implementation order

1. **Container module and clamps selection:** add a reusable optional module and
   select it only on clamps. Start a fresh official release with persistent
   configuration, host networking and no radio/device permissions by default.
   Exit: empty-instance readiness, repeat apply and reboot in a named clamps VM.
2. **Private HTTPS:** extend generic Caddy upstreams for a host-network listener,
   retaining current bridge consumers. Use the existing environment DNS/TLS and
   verified tailnet-user policy; use shared-owner login with native password fallback.
   Exit: onboarding/login, WebSocket/API and companion-client flows work;
   external clients and direct port 8123 cannot bypass the proxy policy.
3. **Backups:** configure daily encrypted native backups, initially seven local
   copies; atomically publish completed original archives into
   `/var/lib/data/syncthing/data/backups/homeassistant`. Explicitly share that
   folder or include it in an existing declared share. Reuse the `syncthing`
   btrbk leaf and independently retained off-machine copies.
   Exit: encrypted backup, completed-only publication, peer receipt and snapshot
   inclusion verified; recovery password retained in KeePassXC.
4. **Restore rehearsal:** use synthetic integrations and a separately owned,
   isolated VM. Restore from both a replicated archive and snapshot copy.
   Exit: settings, test automation and database state return, wrong-key refusal
   works, and reboot/reapply preserve state without contacting real devices.
5. **Physical activation:** deploy on clamps, complete onboarding and confirm
   the first actual device/integration inventory. Implement selected companion
   services through the linked plan. Exit: real discovery/control, phone access,
   monitoring and encrypted backup/off-machine recovery readiness confirmed.

No existing Home Assistant installation needs importing. No MQTT broker, Zigbee
bridge, public webhook route or voice stack is assumed in this first slice.
Their selection depends on actual devices/integrations. This plan does not
select Home Assistant on joey or donbot, or require new workstation packages.

## Ownership, inputs and upgrade policy

- Skillet owns the Quadlet, storage preparation, proxy declaration and backup
  export/monitoring workflow. Home Assistant owns users, integrations, automations,
  its recorder database and application settings after onboarding.
- Reuse existing private UI domain/zone and user allowlist; no second Cloudflare
  token-creation path. Keep real names, device identifiers and credentials private.
- Home Assistant onboarding creates native accounts. Recovery material and any
  exporter API token use the existing KeePassXC workflow, with exact requirements
  added to the shared audit catalog/checklist during implementation. Missing
  optional device/integration secrets do not enable an unselected companion service.
- Keep live `/config` outside Syncthing. Publish backups only; never mirror the
  running SQLite database or `.storage` tree as the recovery mechanism.
- Initial application release is pinned and automatic container updates disabled.
  Upgrades require a completed independent backup; image rollback alone may not
  reverse database migrations. OS updates remain under existing fleet policy.
- Host networking needs explicit listener isolation and correct Caddy proxy trust;
  container DNS cannot resolve this service as an ordinary bridge upstream. The
  minimal working SELinux/capability policy is a runtime acceptance result.

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
  a host-loopback listener. Preserve existing Pi-hole/Syncthing payloads through an
  explicit compatibility path if the credential schema changes. Keep host,
  environment, instance and SSH target separate.
- Declare `homeassistant` and derive its hostname/aliases through the existing
  generic machinery. Reuse the selected environment's zone, UI prefix, token
  issuance, A/AAAA/CNAME reconciliation and owned disposal.
- Keep Caddy on its bridge. Implement and verify the selected Unix-to-loopback relay; keep the HTTP listener
  loopback-only and deny raw network TCP 8123 in both families. Configure
  Home Assistant's forwarded-header trust for the actual proxy source and
  restrict raw listener access so LAN or tailnet clients cannot bypass Caddy.
  Network discovery must still operate independently of HTTP listener policy.
- Inspect the pinned release's supported initial HTTP/proxy configuration.
  Use its upstream config generator and retain included automation, script and
  scene files. Initialize confirmed initial HTTP settings through its own store
  API; unconfirmed YAML migrations revert after five minutes. Do not
  repeatedly overwrite `.storage` or assume a legacy YAML block remains the
  convergence interface. Document any required one-time onboarding step.
- Retain the verified tailnet-user gate and use the native trusted-network
  provider to select the shared HA owner for forwarded tailnet clients without
  another password. Keep native password fallback for local recovery; do not
  trust LAN/loopback for automatic login or overwrite native account credentials.
  Validate companion app login/refresh and WebSockets. No public ingress or webhook
  exception is selected. Do not add OAuth proxy, Tailscale
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
  Verify the selected release's effective local backup directory before exporting.
  Current Container archives are under `/config/backups`, corresponding to
  `/var/lib/data/homeassistant/config/backups` on the host.
- Add a bounded Rust export workflow using existing effect boundaries. Establish
  successful completion through the supported backup state/API, not file age,
  a guessed sleep, or an archive filename. Read original encrypted archives;
  the ordinary UI download decrypts backups and is unsuitable for this export.
- Copy a completed archive to an ignored temporary file in the destination
  filesystem, verify bytes, then rename atomically into
  `/var/lib/data/syncthing/data/backups/homeassistant`. Interrupted work must be
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
  copy. Initial publication performs no automatic deletion; monitor disk space and
  establish independent receiving retention before selecting export pruning. Any
  later pruning must preserve the last verified recovery copy.
- Retain only the native encryption password in KeePassXC and independently
  back up the encrypted vault; keep archives outside it. If exporting requires an application API token,
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
limitations in `../plan/archive/ACCEPTANCE.md`.

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
