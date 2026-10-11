# Fleet host declarations

Boxy is replaced by **joey**; rupik by clamps; calculon by donbot.
The host profile is the authority for services, UI routes, credentials, boot
expectations and acceptance. Shared recipes own behavior; host callers opt in.
Joey selects Pi-hole, Syncthing, Tailscale, btrbk, DDNS, Datadog and shared
SMTP. UniFi remains selected only where declared.

Decision, 2026-10-11: use joey for boxy's replacement and donbot for
calculon's replacement. The rename deliberately provides no selector, package
or image aliases. Current CLI selectors, binary/package names, templates and
image builds use these identities. Do not silently rename retained VM manifests,
provider ownership journals or vault entries; historical acceptance preserves
the identities actually tested. Existing disposable runs need an explicit
owned cleanup/migration path; new provisioning uses the renamed profiles and
requires the corresponding images to be built and published first.

Both servers share the Btrfs data layout and credential-gated full-apply unit.
The full unit applies to profiles requiring Pi-hole and Tailscale credentials;
future capability sets must select an appropriate activation unit. Per-host
includes select artifacts/images; generic units read `/etc/skillet/host`.

Boxy's subnet-routing and private cross-host routes/probes need separate policy
and private inventory. Selecting the current shared modules does not claim full
live source parity. See [the migration plan](../plan/JOEY-MIGRATION.md).

Calculon/donbot source services, exclusions and public/mixed ingress needs are
in [the inventory](../plan/DONBOT-INVENTORY.md) and
[migration plan](../plan/DONBOT-MIGRATION.md). The donbot image recipe exists;
its Skillet profile, host binary and Butane configuration remain pending.
