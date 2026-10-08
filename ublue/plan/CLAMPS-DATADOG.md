# Datadog port

Status, 2026-10-08: rootful Agent, credential delivery and caller-derived boot
ordering are implemented. Two reboots of `clamps-test-discovery` automatically restored one Syncthing
HTTP and four process checks, without an Agent restart. Offline checks pass
(301 tests, Clippy, formatting, Fedora repeat apply), and named-VM smoke passed.
Owned Cloudflare/Tailscale cleanup and VM disposal passed. eBPF tracing
and richer metrics remain pending; see `../butane/ACCEPTANCE.md`.

Prior validation, 2026-10-07: formatting, pedantic Clippy, 299 workspace tests,
Fedora base repeat apply, and named-VM Agent startup/delivery passed. The earlier
missing KVM observation was sandbox visibility; native session libvirt works.
`clamps-test-monitoring` confirmed Agent 7.84.1, SELinux enforcing, hostname,
valid API key, successful submissions, default host/container checks, Syncthing
HTTP health and four process instances, and no-op container identities.
Live acceptance found and fixed socket-as-directory convergence, HTML escaping
of Quadlet labels, and shadowing of built-in integrations. eBPF tracing remains
failed: perf probe attachment returned “function not implemented” and fallback
registration could not write to read-only tracing files. Review the required
kernel/seccomp/tracing access before claiming network-monitoring parity.
Named-VM smoke passed. Both optional services and submissions recover after
reboot, but HTTP/process Autodiscovery does not: restarting the Agent restores
one HTTP and four process checks. Fix boot discovery/reconciliation before
claiming application monitoring recovery. Owned Cloudflare/Tailscale cleanup
and VM disposal passed; evidence is in the acceptance log.
Private ping targets and richer application metrics remain additional work.

Read `../AGENTS.md`, `../skillet/AGENTS.md`, `../design/monitoring.md`,
`../design/secrets.md`, and `../design/configuration-templates.md`.
Install nothing on the workstation. Keep routine CI offline and vault-free.

## 1. Module declarations

Implemented: shared `skillet_datadog` Autodiscovery renderer, consumed by each
container recipe. Syncthing owns an internal unauthenticated health probe;
Pi-hole, DDNS, UniFi, Caddy, and Tailscale own process-liveness declarations.
Labels contain public configuration only. They are inert without an agent.

Local validation, 2026-10-06: all 284 workspace tests, pedantic Clippy and
formatting passed on the musl target. The installed Quadlet generator accepted
the passive fixture and retained `%%%%host%%%%` in ExecStart, which systemd
expands to Datadog's `%%host%%`. The CI Fedora base scenario passed two applies
without service start/restart on repeat apply. No live agent checks or telemetry
ingestion were exercised.

- Validate labels through the actual Quadlet generator, especially escaping of
  Datadog `%%host%%` variables through generated systemd ExecStart specifiers.
- Cover JSON quoting and actual service declarations in CI.
- Keep discovery independent of fixed container IPs or publicly exposed UIs.

Exit: renderer tests and actual Quadlet generation preserve the JSON and
Autodiscovery placeholders; relevant local CI checks pass.

## 2. Agent and host profile

Implemented: official Agent container as an optional canonical host service.
Clamps opts in. Agent image index evaluated for this port:
`registry.datadoghq.com/agent@sha256:161a43ad2b290f7527a70e70c8880c1b3d65b39411552a8cb41398049546daf5`.
Pinned metadata and live runtime both report Agent 7.84.1.

Reviewable runtime boundary:

- Rootful Agent with host PID/cgroup namespaces and host networking for
  container HTTP probes; no published Agent, DogStatsD, or APM ports.
- Rootful `/run/podman` directory mounted read-only, exposing `podman.sock`, for supported Docker API
  Autodiscovery. Enable/start `podman.socket` only for the opted-in agent. The
  socket can mutate containers despite the read-only bind mount.
- Read-only `/proc`, `/sys/fs/cgroup`, `/run/systemd`, and host filesystem mounts
  for process, Btrfs, disk, and systemd data. Never relabel host paths with `Z`.
- SELinux label separation disabled for this monitoring container if required
  by the evaluated runtime; record the reason and actual uCore test evidence.
- Explicit `network_monitoring` caller policy. To retain Chef's eBPF network and
  service monitoring, add the documented system-probe capabilities:
  `SYS_ADMIN SYS_RESOURCE SYS_PTRACE NET_ADMIN NET_BROADCAST NET_RAW IPC_LOCK CHOWN`
  and required debugfs access. Do not enable unrelated APM/log collection.

Use the Podman-socket backend first. Do not infer the standard Podman graphroot:
this fleet stores containers under `/var/lib/data`, and any DB-backend alternative
must use the authoritative storage configuration. Verify bridge-network probes
and eBPF packet visibility before claiming parity.

Exit: optional eligibility, required data mount, generated privileges/mounts,
no plaintext credentials, no-op repeat apply, secret rotation, and failed-start
recovery are covered by local tests. Ordinary base/full apply and smoke do not
require Datadog credentials or start telemetry.

## 3. Configuration and delivery

Implemented: template, schema validation, native Podman environment secrets,
separate credential-gated phase, production delivery, explicit disposable
enrollment, and authoritative `env:prod`/`env:test` tags. The official image's
startup requires `DD_API_KEY`; file-secret injection is not used. Live encrypted delivery, API-key validation and successful submissions passed;
account-side graph/dashboard acceptance remains separate.

- Use the public `datadog` template with one typed secret reference to
  `skillet/datadog/api-key`, shared by production and test. Site is ordinary
  template configuration, defaulting to `datadoghq.com`. Host overrides can
  select a different site and private tags. Do not require an application key for
  ingestion. Version and validate the rendered payload before delivery.
- Add a shared `--phase datadog`, `skillet-datadog-apply.service`, and
  `datadog_config` encrypted credential. Production command:
  `secret deliver <host> datadog`. Use existing verified stdin-only transport.
- Mount API keys as native Podman secrets, preferably files if supported by the
  pinned image; native secret environment injection is an explicit supported
  fallback. Never use literal `Environment=DD_API_KEY=...`.
- Disposable enrollment is explicit through `--with-datadog`, with the shared
  key, `env:test` tagging, and the actual VM hostname; record that remote
  telemetry/host retention is distinct from VM/token cleanup. Use dummy
  credentials and disabled networking for offline checks.

Exit: successful template validation, deferred activation, rotation, and missing
credential handling; no vault/API requirement in routine CI.

## 4. Checks and acceptance

Follow-ups from live acceptance:

1. Implemented: order Agent startup after caller-declared monitored units,
   without activating optional consumers. Pinned workload metadata/provider
   source uses an initial inventory followed by events, not periodic collection.
   Offline tests cover caller-derived order, absence of activation dependencies,
   and self-dependency rejection. Both live reboots and repeat apply passed;
   named-VM smoke passed. Later runtime event
   reconciliation remains outside this boot fix's evidence.
2. Resolve eBPF probe attachment separately. Inspect effective seccomp, kernel
   support and tracefs/debugfs access; use official Agent requirements and the
   narrowest supported runtime policy. Test with real uCore SELinux and report
   Secure Boot state explicitly. Do not claim parity from an active container or
   host interface counters. Document any additional privileges before enabling
   them; do not silently broaden the Agent boundary.


- Agent owns shared host system/process/network/Btrfs metrics and localhost SSH
  TCP check. Aggregate systemd unit contributions from the canonical service
  declarations, including Btrbk's timer; do not maintain a second service list.
- Configure private ping targets through templates and reviewed integration
  dependencies. Preserve retained Chef tags privately after inventory.
- Verify Syncthing's no-auth health endpoint on the deployed image. Later add
  synchronization metrics through a reviewed supported integration or exporter;
  do not claim that HTTP health establishes folder/device synchronization.
- Review Pi-hole 6 authentication and community-check compatibility before
  installing it. DNS-query checks and API statistics must have separate evidence.
- DDNS process liveness is insufficient to prove publication; keep its separate
  ownership/publication acceptance evidence and production cutover plan.
- Named VM: verify Agent status, detected containers, check success, host rather
  than container metrics, Btrfs data, systemd units, eBPF under Secure Boot,
  no-op/reboot recovery, and explicit test ingestion. Record results in
  `../butane/ACCEPTANCE.md`; report unavailable checks honestly.

Exit: retained Chef capabilities and deliberate changes reconciled in parity
docs. Rich application checks remain explicit if acceptance is unfinished.
