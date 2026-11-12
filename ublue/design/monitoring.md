# Monitoring composition

Retain Datadog for current fleet parity. Services own monitoring declarations;
the optional Agent consumes the same caller-selected inventory as application
composition. No separate monitor registry or credential-bearing discovery labels.
Container/process liveness and application correctness remain distinct checks.

## Agent boundary

Use the official rootful Agent `:7` channel with Quadlet registry auto-update.
Patch/minor updates follow the supported major version; major upgrades remain
explicit. Skillet owns runtime integration
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
Wants/Requires dependencies. The collector's initial inventory follows
startup; Podman event delivery alone did not reliably recover concurrent boot
labels. Ordering does not guarantee later event reconciliation. Credentials and
caller opt-in gate telemetry; merely installing a monitored module does not.

The Agent gets its own unfiltered DNS resolver list (default Cloudflare public
resolvers) through Quadlet rather than inheriting Pi-hole. Monitoring must keep
reporting when its observed DNS service is unavailable or filters telemetry.
Resolvers are typed clear template configuration; retained payloads receive the
same defaults. Host/client filtering stays unchanged and strict TLS validation
remains enabled. DNS sinkhole replies can otherwise send intake HTTPS to the
local proxy and masquerade as a TLS failure.

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

Guest Datadog apply derives the `ucore-kexec` tag from the canonical host's kexec
selection. It strips stale/conflicting copies from retained payloads, so templates
cannot claim that capability. Uptime monitors select `env:prod,ucore-kexec` and
group by host rather than maintaining a host list. Tag presence describes declared
policy; uptime remains the check that scheduled activation actually runs.

Commands: [Datadog](../skillet/crates/datadog/README.md),
[NUT](../skillet/crates/nut/README.md). Remaining acceptance/enhancements:
[monitoring plan](../plan/CLAMPS-DATADOG.md).

Sources: [collector](https://github.com/DataDog/datadog-agent/blob/8e795c5e/comp/core/workloadmeta/collectors/internal/docker/docker.go),
[Podman deployment](https://docs.datadoghq.com/containers/guide/podman-support-with-docker-integration/),
[Autodiscovery](https://docs.datadoghq.com/containers/docker/integrations/),
[network monitoring](https://docs.datadoghq.com/network_monitoring/cloud_network_monitoring/setup/),
[DogStatsD](https://docs.datadoghq.com/extend/dogstatsd/).

## Planned switch collectors

Clamps will collect optional NETGEAR Plus telemetry through per-device Podman
collectors containing pinned `py-netgear-plus` dependencies. Private OpenMetrics
endpoints feed the existing Agent; collectors receive only their switch password
and no Datadog API key. Switch identity is explicit rather than inherited from
the collection host. These are custom metrics, not automatic Network Device
Monitoring inventory. Failed polls expose failure without fresh device readings.
Expected-speed alerts apply only to configured uplink ports. Implementation and
physical firmware compatibility remain in the
[switch monitoring plan](../plan/NETWORK-SWITCH-MONITORING.md).

The selected NETGEAR library uses cleartext HTTP for switch login and statistics.
Accept that only for a private or link-local IP on the trusted LAN; do not accept
public addresses or imply TLS support. The management endpoint and password stay
in KeePassXC and are delivered only to that switch's collector.

HA state export uses the built-in Datadog integration and host-loopback
DogStatsD UDP. Canonical host composition opts the Agent into this listener only
when HA is declared. The Agent owns listener configuration and ingestion keys;
HA owns its UI-managed integration state. Unix-socket collectors remain unchanged.
