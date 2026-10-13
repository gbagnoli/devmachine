# Monitoring composition

Retain Datadog for current fleet parity. Services own monitoring declarations;
the optional Agent consumes the same caller-selected inventory as application
composition. No separate monitor registry or credential-bearing discovery labels.
Container/process liveness and application correctness remain distinct checks.

## Agent boundary

Use a pinned official rootful Agent container. Skillet owns runtime integration
configuration/activation; the OS supplies Podman/kernel tooling. Community checks
requiring extra dependencies need a reviewed image rather than runtime downloads.
A shared vault API key serves both environments; site is public template policy.
Private tags and credentials reach Podman secrets, not Quadlet Environment literals.

Host PID/cgroup/proc/filesystem visibility and explicitly selected eBPF capabilities
support host/network monitoring. Preserve default seccomp, explicit PERFMON and
read-only debugfs; SYS_ADMIN alone is not a perf_event_open allow rule. Secure
Boot/SELinux acceptance is separate from BIOS VM acceptance. Mount the rootful
Podman API directory without converting its socket into a directory; a read-only
socket bind still grants API authority. Mount integrations individually to retain
upstream default checks.

Order the Agent after caller-selected monitored units without activation
Wants/Requires dependencies. The pinned collector's initial inventory follows
startup; Podman event delivery alone did not reliably recover concurrent boot
labels. Ordering does not guarantee later event reconciliation. Credentials and
caller opt-in gate telemetry; merely installing a monitored module does not.

## Native collectors

Native modules may use the existing static guest binary to submit bounded
DogStatsD metrics. The unchanged official Agent owns a Unix datagram socket in a
root-only volatile directory; UDP collection is disabled. Collectors have no
API key/vault access and do not activate the Agent or optional application.
Successful Agent apply configures selected module timers.

NUT exports a finite numeric/status inventory using bounded read-only queries.
Failed observations emit communication failure, not invented zero readings.
Missing optional measurements are omitted; host/environment/location tags follow
the Agent. Unix submission proves local receipt, not cloud ingestion. Bounded
metric/tag inventory controls custom-metric volume.

## Environment and notification policy

Delivery sets authoritative env:prod or env:test. Production region comes from
the caller; all disposable telemetry uses region:lab. Monitor data queries exclude
env:test while retaining legacy production hosts. Monitor metadata tags are not
data filters. Optional monitor administration uses a separate workstation-only
scoped application key, never an Agent credential. VM disposal does not remove
historical telemetry.

Commands: [Datadog](../skillet/crates/datadog/README.md),
[NUT](../skillet/crates/nut/README.md). Remaining acceptance/enhancements:
[monitoring plan](../plan/CLAMPS-DATADOG.md).

Sources: [collector](https://github.com/DataDog/datadog-agent/blob/8e795c5e/comp/core/workloadmeta/collectors/internal/docker/docker.go),
[Podman deployment](https://docs.datadoghq.com/containers/guide/podman-support-with-docker-integration/),
[Autodiscovery](https://docs.datadoghq.com/containers/docker/integrations/),
[network monitoring](https://docs.datadoghq.com/network_monitoring/cloud_network_monitoring/setup/),
[DogStatsD](https://docs.datadoghq.com/extend/dogstatsd/).
