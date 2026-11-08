# Dependency maintenance

The November 2026 security update removes the vulnerable `users`, `serde_yml`
and `libyml` dependencies, using `uzers` and the YAML organization's
`yaml_serde`/`libyaml-rs`. Account lookup and YAML consumers retain their existing
interfaces. Patched lockfile versions address the TLS, random-number, Markdown,
constant-time and async-runtime advisories. Python dependencies are outside this
change.

Cargo audit reports zero vulnerabilities, with four maintenance-only warnings:

- `basic-toml` through Askama 0.16.1's configuration parser.
- `zbus_names`, `zvariant`, and `zvariant_derive` through zbus 5.19's blocking API.

Pending: replace these upstream paths when maintained replacements are available.
Moving to zbus 6 requires adapting its removed blocking interface; treat this as
a separate adapter change with live systemd/DBus acceptance. Preserve template
loading when replacing Askama's configuration backend. Do not add advisory ignores.

Exit criteria: the resolved graph has no maintenance warnings, workspace checks
pass, and account, YAML, template and systemd behavior remain verified.
