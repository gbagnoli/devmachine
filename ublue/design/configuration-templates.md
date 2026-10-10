# Configuration templates and secret references

Decision, 2026-10-06: keep ordinary service configuration as clear, reviewed
source templates; keep only host/environment-specific leaf values in KeePassXC.
This avoids storing and hand-editing whole JSON documents in the vault while
keeping private values out of the public source.

Templates are JSON keyed by service, with a shared default and optional complete
host overrides. A host override replaces the entire shared template; automatic
field or array merging is intentionally undefined. A value can use the typed
marker `{"$secret":"skillet/environments/{environment}/hosts/{host}/..."}`.
Skillet substitutes only the validated host and environment components in that
entry path, then replaces the whole marker with the KeePassXC Password value as
a JSON string. Secret contents are never interpolated into surrounding text or
interpreted recursively.

The workstation resolves references in memory before service-schema validation
and before provider mutations. Missing entries, malformed references, and
unknown placeholders in entry paths fail closed. Ordinary strings remain
literal. Rendered documents and secret values are not
logged, recorded, or persisted as workstation files. Guest apply has no vault
dependency.

The embedded catalog is
[`configuration-templates.json`](../skillet/crates/workstation/src/configuration-templates.json);
the resolver is workstation-only. Adding a service adds its default template to
the catalog; add a host override only when a caller needs a different complete
configuration. For example, DDNS keeps policy in its template and its relative
record name in
`skillet/environments/<environment>/hosts/<host>/cloudflare/ddns-dns-name`.

When converting an existing vault-held JSON document, move its structural policy
into the source template and each private leaf into its own KeePassXC entry.
For DDNS, move `name` to `ddns-dns-name`; keep `proxied` and
`takeover_existing` in the template. Multi-record hosts can provide a complete
host override with one typed reference per record. Remove the obsolete whole
document entry after the new template and leaf values have been validated.

Datadog resolves one shared `skillet/datadog/api-key` for both environments.
Its site is ordinary template configuration, defaulting to `datadoghq.com`,
with optional host overrides/private tags. Delivery
adds the selected environment tag and rejects a conflicting one; credentials
alone never select the Agent capability.
