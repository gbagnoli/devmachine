# Datadog

The caller must declare Datadog. Run Cargo commands from `ublue/skillet`.

## Setup

Create `skillet/datadog/api-key` in KeePassXC Password: one shared ingestion API
key for both environments. No application key is needed for ingestion.
Site is public configuration in the `datadog` template in
[configuration-templates.json](../workstation/src/configuration-templates.json),
defaulting to `datadoghq.com`. Host overrides/private tags follow the
[template format](../../../design/configuration-templates.md).

Delivery sets `env:prod` or `env:test`; production region comes from the caller,
and tests use `region:lab`. Exclude `env:test` in monitor **data queries**, retaining
legacy production coverage (`env:home` or no environment tag). Metadata tags do
not filter telemetry. Optional workstation monitor administration uses
`skillet/datadog/monitor-setup-key` in Password, with `monitors_read` and
`monitors_write` application-key scopes. Never deliver that key to the Agent.

The official `registry.datadoghq.com/agent:7` image follows Agent 7 updates through
`podman-auto-update.timer`. No OS reboot is needed for an Agent update.

`dns_servers` is clear template configuration, defaulting to `1.1.1.1` and
`1.0.0.1`. Use an independent unfiltered resolver; the Agent must still report if
Pi-hole fails or blocks a Datadog intake hostname. Host/client resolvers are
unchanged. Override the nonempty IP list in the same template when needed, then
redeliver Datadog configuration. Retained credentials use these defaults when the
field is absent. TLS certificate verification remains enabled.

## Provision

```sh
cargo run --release -p skillet -- secrets deliver HOST datadog --target USER@ADDRESS --activate
# Existing owned disposable VM:
cargo run --release -p skillet -- test vm provision HOST INSTANCE --with-datadog
```

[Remote apply](../workstation/README.md#apply-a-production-host) includes all
selected services. Ordinary VM provisioning does not enable Datadog. Close
KeePassXC while generated credentials may be saved. VM disposal does not erase
historical Datadog telemetry.

Inspect `datadog-agent.service`, `skillet-datadog-apply.service` and their journals.
Guest credentials are encrypted; the API key/private tags reach the official
rootful Agent through Podman secrets. Modules supply their own monitoring.
Hosts with NUT also configure its [collector timer](../nut/README.md#metrics).

Design: [monitoring](../../../design/monitoring.md).
Acceptance backlog: [Datadog plan](../../../plan/CLAMPS-DATADOG.md).
