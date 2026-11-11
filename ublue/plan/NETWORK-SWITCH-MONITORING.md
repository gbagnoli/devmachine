# Network switch monitoring on Clamps

Status: in progress. Workstation-side typed inventory resolution, ownership
checks, dynamic Vault auditing, generated secret documentation and encrypted
Datadog payload delivery are implemented. A preliminary read-only collector and fixture tests exist in the Datadog crate;
image packaging and initial guest integration are implemented. The collector
publication workflow runs daily and on default-branch source changes; registry
publication itself and managed production deployment remain unverified. Temporary
physical-switch polling and VM Agent parsing passed; alerts remain pending.
Clamps is the production collection host, independently of the workstation used
for configuration delivery. Preserve existing Datadog and NUT acceptance in
[Clamps monitoring](CLAMPS-DATADOG.md).

## Scope and decisions

Add optional switch monitoring through Skillet's existing host capability,
workstation Vault, encrypted delivery, Podman and Datadog composition. Support
only NETGEAR Plus switches initially; reject unsupported makes explicitly.
Do not configure switch ports, change PoE, upgrade firmware or reset switches.

Use a dedicated collector image containing a thin Python wrapper and a pinned
`py-netgear-plus` release with pinned dependencies. No upstream production image
was identified; verify packaging before implementation. Do not install packages
at container startup. Publish through the reviewed source/image workflow, keeping
generated bundles and private inputs out of Git.

The initial physical target is a GS308EP with firmware 2.0.0.11. Upstream lists
2.0.0.5 as tested; compatibility with 2.0.0.11 remains a physical acceptance gate.
Do not commit private IPs, MACs or credentials in fixtures or documentation.

The upstream library documents login and statistics requests over plain HTTP.
Before sending the UI password, verify this management endpoint is restricted to
the trusted LAN path. Treat credential confidentiality on that LAN as a required
deployment assumption unless the library's HTTPS support is proven.

## Vault and configuration contract

Use one inventory entry per collection host and one credential entry per device:

- `hosts/clamps/network/switches`: custom field `switches` is a comma-separated
  list of stable device names, for example `office,router_uplink`. Names use
  lowercase letters, digits, hyphens or underscores; they start with a letter
  and end with a letter or digit.
- `network/switches/<name>`: standard Password is the UI password; standard URL
  is the management endpoint; custom field `make` is `netgear`. Username is
  optional and unused by the initial NETGEAR adapter.

Resolve these default references through existing configuration/template
mechanisms, including the repository's vault namespace conventions. Each adapter
owns its required fields and their interpretation. Stable device names survive
address changes and collector reassignment. Poll intervals, expected port speeds
and port labels belong in typed Skillet configuration rather than Vault fields.

Missing inventory or missing `switches` means disabled; an explicitly empty list
also requests removal of previously managed collectors. Distinguish an absent
entry from Vault access failure. Never interpret lookup/authentication errors as
an instruction to deactivate. Validate the complete inventory before mutation:
trim names, reject invalid/path-traversing names, duplicates, missing entries,
unsupported makes, invalid management URLs and missing/empty passwords. Preserve
password contents exactly. Enforce one production collector owner per device.

This is a narrowly scoped, user-requested exception to the usual rule that
credential presence never enables capabilities: the inventory field enables
switch monitoring only for a caller that already declares Datadog. Keep one
canonical resolved inventory for capability, audit, delivery and runtime use.
Disposable environments never inherit production switch targets or passwords.

## Implementation sequence

1. Add typed inventory/configuration behind the workstation Vault boundary.
   Implemented in `skillet_workstation::switch_inventory`: validate stable
   names, private management URLs, supported make, passwords and one-owner
   inventory while reading other hosts' names without their passwords. Test
   policy does not look up production entries. Dynamic Vault audit reports the
   exact required fields for configured switch entries; the root checklist is
   generated from the shared schema. Delivery places the resolved records only
   in the encrypted Datadog credential payload. An absent inventory remains
   unmanaged; an explicitly empty one is a removal request. Fixture tests
   verify test payloads do not retain production addresses or passwords.
2. Finish guest lifecycle delivery: interpret the optional/explicit-empty
   selection, stage per-switch Podman secrets, run collectors, render the Agent
   OpenMetrics configuration, and safely retire removed collectors and secrets.
   Keep idempotent recovery for interrupted updates. This remains pending.
3. Package a read-only collector with bounded network operations, session reuse
   where supported, retry backoff and approximately 30-second polling. Run one
   Quadlet container per switch with a private OpenMetrics endpoint reachable by
   the existing Agent, without publishing a LAN listener. Isolate failures between
   devices and verify container-to-switch routing on Clamps.
4. Add a credential-free OpenMetrics declaration owned by the monitoring module.
   The existing Agent retains the Datadog API key and handles ingestion. Do not
   activate an optional Agent merely through collector dependencies. Reconcile
   inventory additions, removals, secret rotation and interrupted activation
   idempotently, removing obsolete collectors and their runtime credentials.
5. Define a bounded metric inventory: port negotiated speed, link status,
   collection success, last successful observation and supported RX/TX counters.
   Add CRC or PoE only when actually exposed. Verify units against library code
   and the UI; handle counter reset/wrap without negative rates. Failed polls emit
   failure and omit current device readings, never invented zeros or stale values
   presented as fresh.
6. Attribute samples to the switch using an explicit OpenMetrics hostname label
   and stable device identity. Prefer validated MAC-based identity when available,
   with the inventory name as a stable configured fallback. Retain switch name,
   address, make/model, port and collector-host tags plus authoritative environment
   and region policy. Verify Agent host-tag enrichment and avoid accidental
   attribution to Clamps. This supplies custom metrics, not automatic Datadog
   Network Device Monitoring inventory; assess series volume and billing before
   rollout. Prevent duplicate collection across hosts.
7. Configure expected speed only for explicitly selected uplink ports. Alert
   after two consecutive successful polls show an up link below expected speed;
   distinguish link down, switch collection failure and collector/Agent no-data.
   Monitor administration remains workstation-only with the existing scoped key.
   Exclude test telemetry in monitor data queries.

## Acceptance

- Unit/fixture coverage: parsing and dynamic Vault audit; missing versus invalid
  inventory; unsupported make; duplicate ownership; secret redaction; rotation;
  omission/removal; authentication errors; partial device failure; stale readings;
  units and counter reset; stable switch attribution.
- Run `cargo fmt`, configured pedantic Clippy and
  `cargo test --workspace --all-targets` for implementation changes. If image
  workflows change, run YAML parsing, shell checks and actionlint using the image
  repository's pinned lint container. Preserve common-before-host build ordering,
  exact-revision artifact consumption and publication-job signing.
- Run the required Skillet runtime smoke against an explicitly named disposable
  VM with real systemd and Podman, using synthetic switch endpoints and dummy
  credentials. Verify repeat apply, concurrent startup, Agent discovery, reboot,
  credential regeneration, rotation and collector removal. Record snapshots and
  failures; never contact production devices from routine tests.
- On Clamps, verify read-only GS308EP 2.0.0.11 compatibility, UI-to-metric parity,
  private endpoint reachability, real Datadog ingestion, switch identity and
  environment tags. Verify post-reboot recovery separately from VM evidence.
- Exercise slowdown/no-data alerts with fixtures; any physical link interruption
  needs separately authorized maintenance. Do not change production switch
  settings merely to validate monitoring.
- Update module operation documentation and the module index when implemented.
  Record dated results in `plan/archive/ACCEPTANCE.md`; retain all unverified gates.

Design: [monitoring](../design/monitoring.md),
[secrets](../design/secrets.md).
Upstream: [py-netgear-plus](https://github.com/foxey/py-netgear-plus),
[Datadog OpenMetrics](https://docs.datadoghq.com/integrations/openmetrics/).
