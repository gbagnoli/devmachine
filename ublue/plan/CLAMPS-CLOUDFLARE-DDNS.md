# DDNS remaining acceptance and cutover

Guest service, templates, scoped tokens, persistent delivery and journaled
VM ownership/cleanup are implemented. Disposable publication, repeat apply,
reboot and disposal have evidence in [acceptance](archive/ACCEPTANCE.md).
[Previous implementation record](archive/CLAMPS-CLOUDFLARE-DDNS.md).
Current commands/configuration: [DDNS guide](../skillet/crates/ddns/README.md).

## Production handover completed — 2026-11-01

The operator stopped Rupik's updater; its unit is masked and not running.
Clamps now selects production DDNS in its caller profile. The operator explicitly
approved adoption of the configured existing public A record. Scoped credential
delivery and activation passed; Cloudflare reports the target's public IPv4,
expected Clamps ownership marker and configured proxy policy. The one-time
host-specific takeover override was removed, and strict repeat delivery passed.
The target updater is active and its credential apply unit is enabled at boot.
No unrelated application or guest binary was deployed for this handover.

## Remaining

1. Inspect effective private source records/configuration and approve retained
   names, proxy state and zone. Keep public-address records disjoint from UI DNS;
   preserve the evaluated image/schema and provider's 100-A-record limit.
2. Validate deliberate credential rotation: establish the replacement through
   owned persistence, redeliver, verify actual publication/consumer use, and
   retire only the owned superseded credential after success. Preserve retry
   evidence if persistence or delivery fails.
3. Verify target publication after its next planned reboot; repeat delivery
   already passed. Keep the source writer stopped during target operation.
4. Preserve production cleanup ownership. Disposable cleanup must never delete
   production or unrelated records. If rolling back, stop the target writer
   before restarting the source.

Use the existing vault/transport/provider boundaries; no new master key or
manual smoke token is needed. Routine checks remain provider/vault free.
Record revision, owned resources, observations and unverified checks separately.
Designs: [DDNS](../design/cloudflare-ddns.md),
[Cloudflare lifecycle](../design/cloudflare-ui-lifecycle.md).
