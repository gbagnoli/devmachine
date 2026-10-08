# Monitoring composition

Keep Datadog for the Rupik-to-Clamps migration, retaining required host,
process, network, filesystem, and service checks. Caddy replaces nginx, so its
checks need a different implementation. Local metrics/Grafana remain a later
discussion outside this migration.

Decision, 2026-10-06: services own their monitoring declarations. Datadog is an
optional host capability; the agent consumes the checks belonging to the
services selected by the canonical host caller. Adding or removing a service
must also add or remove its monitoring without a second service inventory.

Container modules declare credential-free Datadog Autodiscovery labels. The
shared Datadog library renders and escapes those labels. Checks use discovered
container addresses, so private UI ports stay unpublished. Container/process
liveness and application correctness are different checks; process presence
does not establish DNS publication, synchronization, or controller health.
Non-container services contribute explicit systemd units to the host check.

Boot discovery policy: order the Agent after the same caller-selected monitored
units, using `After=` without `Wants=` or additional `Requires=` dependencies.
The pinned Agent collector lists existing containers once, then consumes events;
Podman's boot-time event path did not recover application labels in acceptance.
Ordering makes the initial inventory follow concurrent application startup and
leaves services without delivered credentials inactive. It does not guarantee
reconciliation of missing events later in runtime. Verify reboot discovery on
real Podman; provider polling is not a supported replacement for the pinned
streaming container provider. Two live reboots restored application checks
without restarting the Agent; see the acceptance record.


Approved agent deployment, 2026-10-07: a pinned official rootful Agent container managed by
Skillet. Datadog recommends container deployment for Podman over native host
installation, which requires additional permission handling. The OS image
continues to provide Podman and kernel tooling; Skillet owns activation,
credentials, and changing integration configuration. Community integrations,
if required, belong in a separately built agent image, not runtime downloads.

Implemented agent runtime and credential delivery. Host monitoring
needs host PID/cgroup visibility, read-only proc/cgroup/filesystem mounts, and
SELinux access. Podman Autodiscovery through the rootful API socket grants
container-management authority even with a read-only socket bind mount.
Preserving Chef's network/service monitoring additionally needs the
system-probe's eBPF capabilities and debugfs access. Those privileges must be
explicit caller policy and tested on uCore with its Secure Boot/SELinux settings.
For network monitoring, explicitly select `PERFMON`: the installed Podman
seccomp profile gates `perf_event_open` on that capability. `SYS_ADMIN` alone
left the syscall returning ENOSYS in the VM. Preserve default seccomp and
read-only debugfs; do not disable syscall filtering or make tracing writable
unless a separate diagnosed requirement warrants it. Named-VM acceptance on 2026-10-08 confirmed perf events, NPM/USM traffic
collection and automatic recovery after reboot with SELinux enforcing. The VM
uses BIOS; Secure Boot remains a physical/UEFI acceptance requirement.

The agent has a separate credential-gated apply phase. Installing Skillet
or declaring a monitored service must not start telemetry without the Datadog
capability and credentials. Use configuration templates to combine public
policy with one shared `skillet/datadog/api-key` in KeePassXC. Site is public
template configuration, defaulting to `datadoghq.com`. The application key
used by Chef's handler is unnecessary for metric submission; future dashboard
or monitor management would be a separate workstation responsibility. Private
tags, endpoint credentials, and ping targets follow the existing encrypted
delivery path and must not appear in labels or recorded files.

Host networking preserves host interface visibility and localhost SSH checks;
Autodiscovery reaches container bridge addresses without published UI ports.
The socket is accessed through a read-only bind of `/run/podman`; the volume
helper converges directory roots, not socket files. Integration directories
are mounted individually so the official image retains its default system and
container checks. The pinned official image
requires `DD_API_KEY` at startup, so Podman injects
API key and private tags through native environment secrets; the public site
uses an ordinary `Environment=DD_SITE` directive. No literal secret-bearing
`Environment=` directives are written. Production and test use the shared key;
delivery adds an authoritative environment tag and region. Production region
is declared by the host caller (Clamps inherits Chef's `region:ftwo`); every
test deployment uses `region:lab`, replacing any template region tags.
Monitor notification policy excludes `env:test` in the data query, preserving
legacy production hosts with `env:home` or no environment tag. Monitor metadata
tags alone do not filter host data. One-time monitor administration uses a
separate optional workstation-only application key; it is never delivered to
the Agent. One-time setup completed on 2026-10-08: the all-host Agent no-data and NTP
monitors exclude `env:test`. Provider validation, updates and query read-back
passed; notification settings were preserved. Other existing monitors target
production hosts explicitly. New monitors must preserve this exclusion.
Agent status and successful submissions confirm the delivered test/lab tags;
provider host-tag read-back returned HTTP 403 with the monitor-only key. Both
monitors returned zero test-host groups before and after reboot. PagerDuty
incident delivery was not separately observed. Test
telemetry requires explicit `--with-datadog`; destroying a VM does not remove
historical telemetry from Datadog.

Implemented: labels for Syncthing HTTP health and process liveness for
Pi-hole, DDNS, UniFi, Caddy, and Tailscale; Agent deployment/delivery and
host/process/network/Btrfs/SSH/systemd configuration. Named-VM acceptance
confirmed host/container checks, Autodiscovery, and successful submissions with
SELinux enforcing. eBPF acceptance now passes with explicit `PERFMON`, default
seccomp and read-only debugfs. Real Syncthing bridge traffic was classified as
HTTP with nonzero byte counters and HTTP aggregates, also after reboot. Pending:
private ping targets and richer
Syncthing metrics and Pi-hole's authenticated API integration. The existing
Pi-hole community integration requires compatibility review against Pi-hole 6.

Sources: [pinned container collector](https://github.com/DataDog/datadog-agent/blob/8e795c5e/comp/core/workloadmeta/collectors/internal/docker/docker.go),
[Podman deployment](https://docs.datadoghq.com/containers/guide/podman-support-with-docker-integration/),
[Autodiscovery](https://docs.datadoghq.com/containers/docker/integrations/),
[network monitoring](https://docs.datadoghq.com/network_monitoring/cloud_network_monitoring/setup/),
[Btrfs](https://docs.datadoghq.com/integrations/btrfs/),
[systemd](https://docs.datadoghq.com/integrations/systemd/).
