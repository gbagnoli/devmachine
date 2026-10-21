# DDNS remaining acceptance and cutover

Guest service, templates, scoped tokens, persistent delivery and journaled
VM ownership/cleanup are implemented. Disposable publication, repeat apply,
reboot and disposal have evidence in [acceptance](archive/ACCEPTANCE.md).
[Previous implementation record](archive/CLAMPS-CLOUDFLARE-DDNS.md).
Current commands/configuration: [DDNS guide](../skillet/crates/ddns/README.md).

Rupik still owns the production public A record. Clamps retains DDNS support
for disposable tests but production delivery is disabled until the one-writer
handover. The existing clamps writer was stopped, its Quadlet retained under a
disabled filename, and its boot apply unit disabled. Restore these only during
step 3, after stopping rupik's writer.

## Remaining

1. Inspect effective private source records/configuration and approve retained
   names, proxy state and zone. Keep public-address records disjoint from UI DNS;
   preserve the evaluated image/schema and provider's 100-A-record limit.
2. Validate deliberate credential rotation: establish the replacement through
   owned persistence, redeliver, verify actual publication/consumer use, and
   retire only the owned superseded credential after success. Preserve retry
   evidence if persistence or delivery fails.
3. Schedule production one-writer handover: stop rupik's updater, preserve its
   approved records, activate the caller-selected target and verify authoritative
   DNS. Keep a rollback route without two active writers.
4. Verify target repeat apply/reboot and final production cleanup ownership.
   Disposable cleanup must never delete production or unrelated records.

Use the existing vault/transport/provider boundaries; no new master key or
manual smoke token is needed. Routine checks remain provider/vault free.
Record revision, owned resources, observations and unverified checks separately.
Designs: [DDNS](../design/cloudflare-ddns.md),
[Cloudflare lifecycle](../design/cloudflare-ui-lifecycle.md).
