# Fleet monitoring

Decision, 2026-10-06: keep Datadog when migrating Rupik to Clamps. Preserve the
required host, process, network, filesystem, and service checks after auditing
the effective Chef configuration. Caddy replaces nginx, so nginx-specific
monitoring cannot be copied unchanged.

Local metrics and Grafana may be added later; that decision and implementation
are outside the current migration. Datadog implementation is pending and follows
the DDNS milestone. Keep credentials and private check targets in the vault.
