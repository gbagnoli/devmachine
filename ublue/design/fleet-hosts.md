# Fleet host declarations

Boxy is replaced by **beelzebot**; rupik by clamps; calculon by bender.
The host profile is the authority for services, UI routes, credentials, boot
expectations and acceptance. Shared recipes own behavior; host callers opt in.
Beelzebot selects Pi-hole, Syncthing, Tailscale, btrbk, DDNS, Datadog and shared
SMTP. UniFi remains selected only where declared.

Decision, 2026-10-11: correct the old beezelbot spelling with no compatibility,
explicitly authorized by the operator. No manifest or vault entry is silently
renamed. Historical acceptance commands preserve the spelling actually executed.
Current CLI selectors, binary/package names, templates and images use beelzebot.

Both servers share the Btrfs data layout and credential-gated full-apply unit.
The full unit applies to profiles requiring Pi-hole and Tailscale credentials;
future capability sets must select an appropriate activation unit. Per-host
includes select artifacts/images; generic units read `/etc/skillet/host`.

Boxy's subnet-routing and private cross-host routes/probes need separate policy
and private inventory. Selecting the current shared modules does not claim full
live source parity. See [the migration plan](../plan/BEELZEBOT-MIGRATION.md).
