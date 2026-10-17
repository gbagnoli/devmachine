# Active plans

Only unfinished implementation and acceptance belongs here. Commands live in
[crate guides](../skillet/crates/host-profiles/README.md), decisions in
[design](../design/README.md), and previous milestones/evidence in
[the archive](archive/README.md). [Acceptance log](archive/ACCEPTANCE.md).

| Area | Current work |
| --- | --- |
| Clamps | [Migration/cutover](CLAMPS-MIGRATION.md), [source parity](CLAMPS-PARITY.md) |
| Networking | [Static profiles and physical adoption](HOST-NETWORKING.md) |
| Encryption | [TPM/recovery acceptance](TPM-ENCRYPTED-ROOT.md) |
| UIs | [Private UI/browser acceptance](GENERIC-PRIVATE-UIS.md) |
| DDNS | [Rotation and production handover](CLAMPS-CLOUDFLARE-DDNS.md) |
| Monitoring / UPS | [Physical Agent and NUT recovery/ingestion](CLAMPS-DATADOG.md) |
| SMTP | [Sender checks and separate enhancements](CLAMPS-EMAIL.md) |
| UniFi | [Production adoption and backup cutover](CLAMPS-UNIFI.md) |
| Refactor | [Parked backend/CI checks and artifact audit](SKILLET-REFACTOR.md) |
| Joey | [Replacement hardware and full-service VM rehearsal](JOEY-MIGRATION.md) |
| Donbot | [Calculon migration](DONBOT-MIGRATION.md), [source inventory](DONBOT-INVENTORY.md) |
| Media cache | [Jellyfin Caddy cache](JELLYFIN-CADDY-CACHE.md) |
| Optional service | [Home Assistant](HOME-ASSISTANT.md), [home automation companions](HOME-AUTOMATION-INTEGRATIONS.md) |

Completed refactor slices, baseline audit, snapshots and storage/credential
milestones are archived. Their remaining gates are retained in the active
trackers above; historical success does not close new host/backend acceptance.
