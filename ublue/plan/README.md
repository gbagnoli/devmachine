# Active plans

Only unfinished implementation and acceptance belongs here. Commands live in
[crate guides](../skillet/crates/host-profiles/README.md), decisions in
[design](../design/README.md), and previous milestones/evidence in
[the archive](archive/README.md). [Acceptance log](archive/ACCEPTANCE.md).

| Area | Current work |
| --- | --- |
| Clamps | [Post-install acceptance/backups](CLAMPS-MIGRATION.md), [source parity](CLAMPS-PARITY.md) |
| Image ownership | [Post-production build-time Skillet, pinned releases and daily reboots](IMAGE-OWNED-CONFIGURATION.md) |
| Networking | [Static profiles and physical adoption](HOST-NETWORKING.md) |
| Firmware | [Clamps supervised firmware/BIOS maintenance](CLAMPS-FIRMWARE.md) |
| Encryption | [TPM/recovery acceptance](TPM-ENCRYPTED-ROOT.md) |
| File browsing | [Quantum replacement and phone trial](FILEBROWSER-QUANTUM.md) |
| UIs | [Private UI/browser acceptance](GENERIC-PRIVATE-UIS.md) |
| DDNS | [Rotation and post-reboot publication](CLAMPS-CLOUDFLARE-DDNS.md) |
| Monitoring / UPS | [Physical Agent and NUT recovery/ingestion](CLAMPS-DATADOG.md), [Clamps switch monitoring](NETWORK-SWITCH-MONITORING.md) |
| SMTP | [Sender checks and separate enhancements](CLAMPS-EMAIL.md) |
| UniFi | [Sustained adoption and independent backups](CLAMPS-UNIFI.md) |
| Refactor | [Parked backend/CI checks and artifact audit](SKILLET-REFACTOR.md) |
| Joey | [Replacement hardware and full-service VM rehearsal](JOEY-MIGRATION.md) |
| Donbot | [Calculon migration](DONBOT-MIGRATION.md), [source inventory](DONBOT-INVENTORY.md) |
| Media cache | [Jellyfin Caddy cache](JELLYFIN-CADDY-CACHE.md) |
| Optional service | [Home Assistant](HOME-ASSISTANT.md), [home automation companions](HOME-AUTOMATION-INTEGRATIONS.md) |

Completed refactor slices, baseline audit, snapshots and storage/credential
milestones are archived. Their remaining gates are retained in the active
trackers above; historical success does not close new host/backend acceptance.
