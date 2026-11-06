# Local UPS protection

Run Cargo commands from `ublue/skillet`.

Clamps opts into the shared NUT module for the CyberPower CP1500 AVR USB UPS
(`0764:0501`). Rebase onto the rebuilt image containing `nut` and `nut-client`,
then run the usual full Skillet apply. Configuration is in `/etc/ups`, with
`MODE=standalone`; the server listens only on `127.0.0.1:3493` and `[::1]:3493`.

The local monitor credential is generated automatically and retained under
`/var/lib/skillet/nut`; do not create a KeePass entry. Its configuration files
are root:nut readable. The packaged driver/server/monitor start when the
matching USB device is attached. Ordinary VMs without that device only converge
configuration. Other host callers can select `ServiceConfig::Nut` with their
own device name, USB IDs and description.

Read status locally with `upsc cyberpower@localhost`. Inspect
`nut-driver@cyberpower.service`, `nut-server.service` and `nut-monitor.service`
with `systemctl status` or `journalctl -u`. After attaching hardware, rerun
Datadog apply to include its native units in monitoring.

## Metrics

For a host declaring both NUT and Datadog, successful Datadog credential delivery
also enables `skillet-nut-metrics.timer`. It polls every 30 seconds through the
existing static guest binary. The collector is compiled into Skillet; the shared
Butane include installs that binary at `/var/usrlocal/bin/skillet`. It is not
baked into the uCore image. VM binary delivery updates the same path. Inspect
the timer and collector with
`systemctl status skillet-nut-metrics.timer` and
`journalctl -u skillet-nut-metrics.service`.
After deploying an updated guest binary to a host with existing credentials,
run `sudo systemctl restart skillet-datadog-apply.service` to converge the
Agent socket and timer.

Read a sample without sending telemetry:

```bash
skillet metrics nut --ups cyberpower --dry-run
```

To query a forwarded server, add `--server 127.0.0.1:3493` (the default).
For an explicit submission, run with sudo and omit `--dry-run`.
`--dogstatsd-socket PATH` overrides the default
`/run/skillet-dogstatsd/dsd.socket`. The Agent uses a root-only socket directory;
UDP collection is disabled. No new KeePass entry is required.

Metrics use the `nut.` prefix: battery charge/runtime/voltage, available
input/output voltage/frequency, load/power/temperature and individual UPS status
flags. Missing optional readings are omitted. `nut.communication` and `nut.power`
are service checks; query failure exits nonzero and submits only communication
failure. Data retain the Agent's host/environment/location tags.

Routine CI exercises protocol parsing, actual TCP/Unix-socket transport, the CLI
and timer composition using local fixtures. Run the isolated official-Agent
consumer check from the workspace after fetching the pinned image:

```bash
cargo test -p skillet --test nut_metrics \
  real_agent_accepts_nut_metrics_over_uds_without_udp -- --ignored
```

It uses Podman, a dummy API key and no container network access. See the
[monitoring design](../../../design/monitoring.md) for the collector decision.

Before relying on protection, verify battery status and arrange a supervised
power-loss/shutdown/restart test on the physical machine. Routine smoke tests
must not run `upsmon -c fsd` or command the real UPS to turn off.
See [the UPS design](../../../design/ups.md).
