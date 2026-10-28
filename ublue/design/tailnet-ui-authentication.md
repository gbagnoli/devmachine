# Tailnet identity for private web UIs

Decision, 2026-10-11: file browsing must not ask for another username/password.
Reuse one caller-selected Caddy identity policy for file browsing and Gokapi
administration, allowing the two intended human users. Private policy values
and application-account mappings are delivered privately, never compiled into
source. An address-range gate alone supplies no user identity.

Authenticate the actual connection through the existing host-network tailscaled
LocalAPI WhoIs. Require a user-owned device and an explicitly allowed complete
user identity; reject anonymous/unknown/tagged sources and lookup failures.
Private authenticated hostnames resolve to tailnet addresses: a browser using
the public IP does not supply a tailnet identity merely because Tailscale is
running. For Gokapi, prefer a private admin hostname and a public sharing hostname
if the selected app version supports their URL/origin behavior. Public routes
bypass the identity gate and never receive an administrative header.
Authorize before proxying every protected request. Never use browser-supplied
forwarded or identity headers to identify the user. Preserve the actual peer
through Podman ingress and overwrite/strip identity headers at Caddy.

Gokapi supports [proxy header authentication](https://gokapi.readthedocs.io/en/latest/setup.html).
Map verified identity to existing application accounts with explicitly chosen
roles; permit no arbitrary account creation. Public sharing requests carry no
administrative identity. For a file UI supporting disabled authentication,
disable its internal login only behind the verified gate and a backend confined
to the proxy path. No UI host port or broadly reachable backend is allowed.
Use a private backend network accessible only through the declared proxy path;
verify isolation from other application networks. Omitted host ports alone do
not establish that boundary. No app password is required for file browsing.
App-native passwords are not the chosen user flow for these services.

Implementation: Caddy's built-in `forward_auth` calls the Skillet Rust gate on
its private Unix socket. The gate calls only LocalAPI WhoIs and never caches
identity decisions. Auth subrequests explicitly clear application query strings
and use `GET /auth`; backend requests retain their original method, path, query
and body. The full daemon socket is available to the root-owned gate,
not to Caddy or application containers. The reviewed gate is therefore trusted
with daemon access; a read-only socket mount would not restrict API operations.
The broker has its own `skillet_tailnet_ui_t` SELinux process type with
explicit file/Unix-socket permissions. A separate loader unit installs the
reviewed CIL before broker startup on SELinux hosts. Only the broker connection
permission is granted to container processes; no permission to arbitrary host
daemons is added. Caddy receives the root-only authorization socket directory,
not the delivered identity policy or tailscaled socket. Systemd filesystem and
address-family restrictions remain in place. The loader skips policy installation
when SELinux is disabled; enforcing hosts retain the same private socket boundary.

This avoids the experimental Caddy plugin and a custom Caddy image. Policy is
part of the encrypted version 3 Caddy payload and converges to a root-only host
file. `skillet/tailscale/ui-users` supplies complete LoginName values separated
by commas or newlines. Both environments share that allowlist; explicit caller
UI declarations select which routes use it. Missing/invalid input fails delivery;
a missing or malformed host policy denies access.

Mikochi runs on a dedicated backend bridge with explicit Netavark strict
isolation. Only it and Caddy join that bridge; Caddy has IP forwarding disabled.
Do not set `Internal=true`: Netavark skips the firewall rules for internal
networks, which allowed direct access in the dual-stack acceptance probe.
The selected bridge permits outbound traffic but blocks forwarded ingress from
other networks. Host root remains trusted; no backend port is published.
Strict isolation is supported by the guest's Netavark 1.17.2, independently of
Podman 6's newer default. Verify the actual bridge boundary during acceptance.
Cross-origin browser requests are rejected before authentication/proxying.

Using the Tailscale client LocalAPI with Headscale is the intended portable
approach, but this does not establish compatible returned identities or authorization.
This is an inference from the client-side integration, not accepted Headscale
support. Hosted Tailscale workstation enrollment remains separate. Do not match
only the username before `@`, infer people from tags or equate a tagged test
server with an authorized human client.

Syncthing canonical and alias routes now share this identity gate and use a
dedicated strict backend, while its native application state is preserved.

Pi-hole now uses the same identity gate and dedicated backend as the other
private UIs. Its DNS port remains published; its web port does not. Current host
callers disable native Pi-hole login only in this mode and do not require a web
password credential. Retained units converge their credential prerequisites from
the same host declaration; obsolete encrypted passwords are no longer loaded.

Mikochi's upstream `NO_AUTH=true` middleware skips JWT parsing, but its refresh
handler still requires the omitted token ID. The browser therefore stays on its
login screen despite working anonymous file APIs. For this caller-selected
frontend only, Caddy answers GET `/api/refresh` with a nonsecret compatibility
marker after identity authorization and origin checks, with `Cache-Control:
no-store`. It grants no app identity or bearer capability: every file/API request
still passes the same gate. Other paths/methods retain normal proxy behavior.
The typed payload declares this compatibility mode and rejects it without the
gate; stale credentials must be redelivered. No upstream image fork is needed.

Current route selection is in the caller; operating setup is in
[Caddy](../skillet/crates/caddy/README.md#identity-protected-uis). Browser/identity
acceptance belongs to [the UI plan](../plan/GENERIC-PRIVATE-UIS.md); Gokapi
integration belongs to [donbot migration](../plan/DONBOT-MIGRATION.md).


Gokapi implementation uses registered native header-auth accounts, a private
administration origin and a separate public sharing origin. Account ownership is
explicit; temporary native API tokens remain in memory. See the
[sharing boundary](public-service-access.md#gokapi-sharing-boundary).
