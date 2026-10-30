# Monitoring remaining acceptance

Rootful Agent, module-owned discovery, credential delivery, authoritative
prod/test/lab tagging and monitor test exclusions are implemented. Native VM
host/container discovery, eBPF traffic and reboot recovery have evidence in
[acceptance](archive/ACCEPTANCE.md). Prior implementation/diagnostic details are
[archived](archive/CLAMPS-DATADOG.md).
Operating guides: [Datadog](../skillet/crates/datadog/README.md),
[NUT metrics](../skillet/crates/nut/README.md#metrics).

## Remaining acceptance

1. Physical steady-state Agent/system-probe acceptance passed on Secure Boot
   uCore with enforcing SELinux, default seccomp, explicit PERFMON and read-only
   debugfs. Agent health and live cloud metric submissions passed after the
   independent resolver fix; system-probe reported fresh network-tracer activity.
   Physical cold-boot recovery remains unverified for this rollout; native VM
   reboot evidence is separate. See the dated acceptance record.
2. Verify selected NUT timer/socket labels, hotplug/absence behavior, repeat
   apply and cold-boot recovery on uCore. Correlate the bounded collector output
   with real cloud ingestion; local UDP-disabled Agent parsing is insufficient.
3. Arrange supervised physical UPS battery/shutdown/restart acceptance. Never
   force a real UPS shutdown from routine tests or unapproved remote checks.
4. If provider host-tag read-back is required, authorize only the necessary
   workstation permissions; the monitor-only key's 403 is not a tag mismatch.
   PagerDuty incident delivery was not separately observed. Retain test exclusion
   in monitor data queries and legacy production coverage.

## Separate enhancements

Private ping targets, richer Syncthing metrics and a Pi-hole v6-compatible API
check require private inputs/compatibility review. Queue-age/send-failure metrics
belong to the [SMTP follow-up](CLAMPS-EMAIL.md). Local Grafana/metrics replacement
is deferred, not required for the current Datadog port.

Adding checks follows caller/module composition; never create an independent
monitor inventory or place credentials in discovery labels. Record actual live
results in the acceptance record. [Design](../design/monitoring.md).
