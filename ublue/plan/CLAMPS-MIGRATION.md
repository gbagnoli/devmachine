# Clamps migration and cutover

Current implementation and source parity are in [the parity checklist](CLAMPS-PARITY.md).
Stock-SSD installation reached signed uCore, an encrypted Btrfs data mount,
TPM unattended boot and base configuration. Operator container observations do
not establish production service-data migration or client reachability.
See [historical evidence](archive/ACCEPTANCE.md) and
[the previous milestone record](archive/CLAMPS-MIGRATION.md).

## Work order

1. Finish [native networking](HOST-NETWORKING.md): verify image Wi-Fi support,
   deploy matching binaries, prepare the private profiles, and prove static
   Ethernet/Wi-Fi adoption and management recovery across controlled boot.
2. Finish [TPM/recovery acceptance](TPM-ENCRYPTED-ROOT.md): temporary passphrase
   cleanup/header refresh, networkless boot and update/rollback. Stock-SSD
   acceptance is separate from final production disk selection/installation.
3. Implement private Pi-hole v6 custom-DNS delivery. The current recipe writes
   static `custom.list`; a reversible `dns.hosts` acceptance probe does not
   verify that production path. Supply private DNS values and migrate Pi-hole
   state only after the target path is verified.
4. Inventory and migrate production data/identities from rupik. Stop source
   Syncthing/UniFi identities before enabling replacements. Retain the source
   SSD and independent backups until rollback/cutover is accepted. Follow
   [UniFi](CLAMPS-UNIFI.md), [DDNS](CLAMPS-CLOUDFLARE-DDNS.md) and
   [private UI](GENERIC-PRIVATE-UIS.md) handover gates.
5. Validate complete production remote apply with matching guest artifacts:
   [operating guide](../skillet/crates/workstation/README.md#apply-a-production-host).
   Source observations of running containers do not verify this orchestration.
6. Verify physical repeat apply, reboot and an approved OS update; client access,
   peer synchronization, UniFi devices/cloud access, and SSH/sudo/ET. Follow
   [baseline parity](CLAMPS-PARITY.md). Approve the production exit node; routed
   traffic exercises remain omitted by operator decision.
7. Complete automated provisioning/recovery before
   [access finalization](../skillet/crates/cli-common/README.md#administrative-access).
   Verify first-login password change and password-required sudo interactively.
   If older media installed `force-pw-change.service`, stop/disable and remove
   that obsolete expiry unit before adopting explicit finalization.
8. Agree an outage/rollback window and final disk; establish off-machine data
   backup retention and rehearse restore before retiring rupik. Local btrbk
   snapshots are not an independent backup.

## Deferred live acceptance batch

Run these with suitable clients and the required private configuration:

- Pi-hole LAN ingress over IPv4/IPv6, UDP/TCP and host firewall, including an
  isolated custom record and local-resolver/bootstrap outage behavior.
- Private UI DNS/HTTPS from actual tailnet clients and Caddy denial from a
  separate network outside the tailnet; guest loopback denial is insufficient.
- Cloudflare production issuance and bounded renewal, preserving owned records.
- Physical UPS shutdown/restart and NUT timer/SELinux/reboot/cloud metric recovery:
  [monitoring acceptance](CLAMPS-DATADOG.md). No forced shutdown in routine smoke.
- Interrupted SSH credential streaming/encryption: preserve or recover the
  usable full credential, retry through the normal owner, and verify consumer
  usability. Atomic-save/activation-failure tests do not cover this interruption.

The SSH-only user-mode VM cannot prove real LAN ingress. Preserve each parked
check explicitly; [refactor acceptance](SKILLET-REFACTOR.md) remains separate.
Broader OS hardening is a placeholder, not an accepted Chef port. Confirm any
retained legacy NAT routes/WOL targets privately rather than guessing them.

## Separate optional work

[Home Assistant](HOME-ASSISTANT.md) is requested but unimplemented and is not a
rupik-retirement gate. Shared SMTP is implemented; [its follow-ups](CLAMPS-EMAIL.md)
and parked UniFi recovery email are separate. [Joey](JOEY-MIGRATION.md) may
proceed independently, preserving its own source and acceptance boundaries.
