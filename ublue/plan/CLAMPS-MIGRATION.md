# Clamps deployment and remaining acceptance

Status reconciled 2026-11-02 from operator confirmations and
[dated evidence](archive/ACCEPTANCE.md). Clamps is installed on its replacement
1 TB SSD. Disk selection, USB preparation and reinstall are complete; there is
no further SSD move to plan. The operator chose a fresh installation without
restoring data from the old stock SSD. The old work order is
[archived](archive/CLAMPS-BEFORE-SSD-RECONCILIATION.md).

## Completed

- Fresh physical installation verified the initial Fedora root and completed the
  TPM transition to signed uCore without manual recovery. Base apply succeeded,
  encrypted Btrfs `/data` mounted at `/var/lib/data`, and no failed units remained.
  Full credential/application delivery subsequently succeeded.
- Wired/Wi-Fi configuration, route preference and reboot/Wi-Fi switching have
  earlier physical acceptance. Keep those observations distinct from checks on
  the replacement installation; [networking](HOST-NETWORKING.md).
- Syncthing is paired on the replacement installation. UniFi was restored from
  Rupik and the operator confirms both APs appear in the replacement UI. Its LAN
  device firewall rules work; [UniFi](CLAMPS-UNIFI.md).
- Production DDNS moved to Clamps, with explicit record takeover and successful
  strict repeat delivery. Rupik's updater is stopped/masked; keep one writer.
- On 2026-11-02 the operator confirmed cold power-on works on the replacement
  installation, accepting unattended physical startup. Networkless boot and
  individual service/metric recovery remain separate checks.
- The operator finalized password-required sudo. Shared deploy/apply supports
  interactive sudo and a thirty-minute scoped kernel cache; SSH-key unlock and
  the three-hour vault cache remain independent.

## Next work

1. Deploy the current artifacts to the finalized host and verify routine
   password-required deployment. Apply service changes separately when approved;
   do not treat a broad apply as authorization for the pending production
   Bluetooth/SELinux policy activation. Verify automatic ET LAN convergence and
   the rebuilt image's ET drop-in/English locale at the next planned boot.
2. Establish backup readiness: verify current HA scheduled backup/export, UniFi
   backup generation, Syncthing off-machine receipt, retention and a restore from
   an independent copy. Pairing alone does not prove receipt; local btrbk snapshots
   are not an independent backup. No stock-SSD state transfer is required.
3. Verify recovery records/header against the current 1 TB volume, independently
   of the old SSD's records. Physical cold power-on is operator-confirmed; finish
   the remaining [TPM checks](TPM-ENCRYPTED-ROOT.md), including networkless boot
   and update/rollback preservation.
4. Complete production steady-state checks: UniFi adoption/WLAN/Site Manager,
   DDNS publication and Datadog recovery after the next agreed reboot, plus
   [networking checks](HOST-NETWORKING.md). Preserve source-controller rollback
   until those and independent backups are accepted. No repeat finalization or
   first-login password expiry is needed during routine updates.
5. Schedule physical UPS/hotplug/cloud-metric acceptance with supervision;
   [monitoring](CLAMPS-DATADOG.md). Firmware/dbx installation stays deferred until
   console/recovery access is available; [firmware](CLAMPS-FIRMWARE.md).

## Deferred acceptance and separate work

- Normal DHCP-provided resolver use and Pi-hole/bootstrap outage behavior remain
  distinct from accepted explicit LAN DNS queries. LAN IPv6 client ingress awaits
  usable LAN IPv6; isolated Pi-hole AAAA serving has passed.
- Retain unverified private-client/identity-denial and certificate-renewal checks
  in [private UI provisioning](GENERIC-PRIVATE-UIS.md). Older browser evidence does
  not accept every route on the new installation.
- Interrupted SSH credential streaming/encryption remains a separate recovery
  check; atomic-save/activation-failure tests do not establish interruption safety.
- Broad OS hardening remains a placeholder. Inventory legacy NAT/WOL shortcuts
  privately rather than guessing requirements. [Source parity](CLAMPS-PARITY.md).
- [Home Assistant](HOME-ASSISTANT.md) and its selected companions are optional,
  with their own hardware/restore acceptance. UniFi recovery email is
  [parked](CLAMPS-EMAIL.md). [Joey](JOEY-MIGRATION.md) and
  [refactor acceptance](SKILLET-REFACTOR.md) remain independent workstreams.
