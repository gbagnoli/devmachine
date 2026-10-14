# Private UI remaining acceptance

Caller-selected services/aliases, environment resolution, Caddy credential sets,
account-scoped Cloudflare issuance, DNS ownership/reconciliation and disposable
cleanup are implemented. Staging ACME/HTTPS and mutation-recovery regression
checks have [evidence](archive/ACCEPTANCE.md); [history](archive/GENERIC-PRIVATE-UIS.md)
retains their original scope. [Caddy guide](../skillet/crates/caddy/README.md).

## Remaining

1. Confirm explicit cross-environment alias-isolation regression coverage through
   planning/delivery; add a missing case if needed. Invalid-alias and relative
   zone-prefix tests alone do not prove complete environment separation.
2. From real allowed tailnet clients, resolve canonical/alias names through their
   actual resolver and inspect HTTPS/browser behavior. Cover both address
   families, DNS-rebinding filtering and identity-protected file operations.
3. From a separate network outside the tailnet, require Caddy's denial response
   for every declared route. The guest's implemented loopback 403 probe is a
   source-matcher check, not this external-access acceptance.
4. Verify production certificate issuance and bounded renewal with the production
   environment/zone. Preserve owned DNS, tokens and certificate state on retries.
5. For Mikochi, verify large transfers, unusual filenames, folder operations,
   mapped ownership, symlink/root containment, reboot, no second login and denial
   of unauthorized users/direct backend access. Existing container/LocalAPI
   results do not establish every browser or containment case.

Keep public/mixed ingress work in [donbot migration](DONBOT-MIGRATION.md), not
this private renderer. Hosted Tailscale enrollment and Headscale compatibility
are distinct; Headscale identity acceptance is not established. Further provider
fault-injection work is not a prerequisite beyond the existing scoped CI cases.

Use owned named VMs and private inputs; routine CI uses fixtures. Record passes
and unavailable checks honestly. Designs: [private access](../design/private-ui-access.md),
[identity](../design/tailnet-ui-authentication.md),
[Cloudflare lifecycle](../design/cloudflare-ui-lifecycle.md).

## Syncthing identity rollout

Syncthing canonical and alias routes declare user identity protection; its GUI
uses a dedicated strict bridge. Physical forwarding, confined SELinux broker
startup/restart, repeat convergence, allowed-client HTTPS and sibling-backend
denial passed in both IP families. Tagged IPv4 HTTPS and unknown/tagged identity
lookups were denied; forged identity headers and untrusted origins were checked.
See the dated [acceptance evidence](archive/ACCEPTANCE.md).

Remaining rollout checks: tagged/disallowed-client HTTPS over IPv6 from a
separate device (the host's IPv6 hairpin probe timed out), browser/REST/WebSocket
behavior, application-policy revocation/daemon outage, full UDP transfer and
reboot. Application state and existing GUI credentials are preserved.

## Pi-hole and Mikochi no-password rollout

Source now derives Pi-hole's dedicated strict backend, identity gate and optional
native credential from the caller declaration. Existing units receive canonical
credential drop-ins; old passwords are preserved but unused by protected hosts.
Mikochi's browser refresh compatibility response is typed, identity-gated and
noncacheable. Old Caddy payloads must be redelivered through normal provisioning.

Acceptance in progress:

1. Completed: strict workspace checks (470 tests), Butane regressions and
   provider-free real Pi-hole retained-password/restart, Caddy refresh/denial and
   Mikochi file-operation tests.
2. Owned native VM application acceptance passed after update, matching test UI
   provisioning, reboot and repeat full/UI apply. Finish capture-based readiness
   and restore current binaries before further development.
3. Completed on physical clamps: matching Caddy policy/full apply, anonymous
   Pi-hole API, separate-client TCP/UDP DNS, sibling-backend isolation and
   tagged-client HTTPS denial in both IP families.
4. An allowed person's browser must open both UIs without an application login.
   Record browser observations separately from API/fixture results.
