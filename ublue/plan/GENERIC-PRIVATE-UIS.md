# Generic private UI provisioning

Status: shared UI declarations, aliases, environment lookup, Caddy payloads,
Cloudflare account-token issuance, DNS reconciliation, and opt-in smoke VM
cleanup are implemented and passed live staging ACME/HTTPS acceptance on
2026-10-02. Unit tests cover ambiguous token creation, retry after partial DNS
reconciliation, and retry after interrupted DNS cleanup. The opt-in UI VM
provision path now probes each UI from the guest's loopback source and checks
Caddy's explicit 403 response. Remaining: denial from a separate network
outside the tailnet and production ACME/renewal acceptance.

## Goal and fixed decisions

Every host declares its UI services. Shared provisioning derives each public
DNS name as `<service>.<host>.<ui-domain>` and delivers the configuration and
Cloudflare credential through the existing encrypted SSH delivery mechanism.
Syncthing is intended on every host; Pi-hole is included only where configured.
The caller selects the environment and declares the services. Caddy, DNS name
derivation, token issuance, and credential delivery must contain no clamps or
smoke special cases. Machine A/AAAA records are shared by all UIs; canonical
UI names and caller-declared aliases use CNAMEs. Every served name needs
certificate coverage and the same Caddy upstream/access policy.

`host` means the host configuration name, such as clamps. It does not mean a
libvirt domain, Tailscale device name, or VM instance. Environment selects
configuration and policy; SSH target selects where to deliver. These are
separate fields. VM commands select the test environment; ordinary delivery
defaults to production. Never infer environment from hostname substrings.

Keep every real UI domain, zone identifier, and credential in KeePassXC, never
in code, committed examples, plans, Ignition, or images. Values are stored in
the Password field, following the existing exact-lookup convention:

| Entry | Purpose |
| --- | --- |
| `skillet/environments/<environment>/dns/ui-domain` | Optional relative prefix; absent means `ui` |
| `skillet/environments/<environment>/dns/cloudflare-zone-id` | Cloudflare Zone ID of the authorized existing zone |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` | Durable token for a persistent host |
| `skillet/cloudflare/token-creator` | Workstation-only token issuer |

Production and test are environment names, not distinct implementations.
Fetch the mandatory Zone ID's domain from Cloudflare and always append it to
the optional relative prefix. `ui.whatever` in zone `example.com` resolves to
`ui.whatever.example.com`; absent means `ui.example.com`, while empty is invalid.
Former full-domain values must be deliberately replaced or removed locally;
never silently treat them as absolute names or rewrite the vault. Cleanup must
resolve the prefix identically and compare the full namespace with recorded
ownership. Dispose of old UI deployments under their original configuration
before migrating. Keep unrelated host secret paths unchanged.

The shared environment policy selects ACME staging for test and the normal
issuer for production. Token scope is Zone Read plus DNS Write for the selected
zone. This permits editing the whole zone, not just the derived names. The
master token stays on the workstation. Persistent hosts use-or-create their
token in KDBX; disposable deployments use a temporary token and recorded
ownership, with no disposable token value persisted in run metadata.

## Read before implementation

- `AGENTS.md`, `skillet/AGENTS.md`, `design/secrets.md`,
  `design/private-ui-access.md`, and `design/smoke-vms.md`.
- `skillet/crates/cli/src/{main,secret_delivery,tailscale}.rs` and the vault
  cache and secret-delivery tests. Reuse the current KDBX writer, conflict
  checks, three-hour kernel cache, SSH host-key verification, and stdin delivery.
- `skillet/crates/cli-common/src/hosts.rs`, `skillet/crates/caddy/src/lib.rs`,
  the Syncthing/Pi-hole resources, and `butane/includes/skillet.bu`.
- Current CI commands in `.github/workflows/ci.yml` and the real VM smoke
  harness. Preserve the pending work; do not reset unrelated changes.

## Implement in this order

### 1. Shared host UI declarations and environment configuration (implemented)

Introduce a typed UI declaration containing a stable service name and its
bridge upstream (container DNS name and port). Each host caller supplies its
enabled services and Podman network through one reusable host definition.
Use that same declaration for Caddy and workstation DNS planning; do not
maintain two lists that can drift. Clamps declares Syncthing and Pi-hole;
beezelbot declares Syncthing only and now applies that service on its own
bridge network. Future hosts can declare services without acquiring a Pi-hole
dependency.
Declaring future services must not require editing Caddy's renderer.

Add a validated environment configuration and one hostname derivation helper.
Resolve the UI namespace from the Cloudflare zone domain plus the optional
relative prefix (default `ui`) using the same helper for delivery and cleanup.
Validate DNS labels, domain length, duplicates, empty service lists, and
upstream inputs before SSH delivery or remote API mutations. Reject schemes,
paths, ports, whitespace, and configuration injection in the base domain.
Validate the selected zone actually contains the base domain; never broaden
scope to another zone automatically.

Exit: a non-clamps host with only Syncthing derives exactly one route; the
same host definition works with either environment. Use reserved example
domains in tests. Missing mandatory Zone ID config fails with the entry path,
without values; an absent relative prefix uses `ui`.

### 1a. Caller-declared UI aliases (implemented; environment isolation tests pending)

Extend each shared UI service declaration with a list of relative alias names
beneath the environment's UI domain; an empty list preserves current behavior.
Allow a `{host}` placeholder only as a complete DNS label. For example,
`sync.{host}` derives `sync.<host>.<ui-domain>`; `sync` derives
`sync.<ui-domain>`. Keep real domains out of caller code. Reject absolute
names, other placeholders, wildcards, and names outside the selected namespace.
Validate all expanded names globally: aliases cannot equal the machine name,
collide with another UI/canonical/alias name, or create loops. Validate before
unlocking for mutations or issuing tokens; environment values require an
unlocked vault before their validation.

Derive one shared DNS plan containing the machine name, canonical UI names,
aliases, and CNAME targets; use the same resolved names for Caddy delivery.
Extend and version the Caddy payload for aliases, validate it against the
receiving caller declaration, and require explicit redelivery for old payloads.
Render each service's canonical name and aliases with the same upstream and
access restrictions. Caddy manages TLS coverage/renewal for every name; aliases
must serve the application directly rather than redirect to a canonical URL.
Certificate file/count layout is not an acceptance requirement.

Exit: a host with zero aliases behaves as before; a host with multiple aliases
serves every name on its declared upstream. Separate tests cover name/target
collisions, invalid expansions, payload mismatch, and alias isolation across
environments. Use only reserved domains in tests. Document application host or
origin allowlists when an upstream requires changes to accept its aliases.

### 2. Generic Caddy configuration and delivery (implemented)

Replace `CaddySites { pihole, syncthing, ... }` with a versioned service-list
payload and explicit issuer policy. Keep the host credential name `caddy_sites`
unless changing it is necessary. Derive hostnames on the workstation from
the shared declaration; validate the payload again on the receiving host.

Remove the clamps guard in Caddy apply. Pass the caller's network and services
into shared apply. Remove hardcoded `/var/usrlocal/bin/skillet-clamps` from
the delivery helpers; resolve the target binary from the validated host
definition. Shared Butane units invoke the generic `skillet` CLI with the
stable host profile in `/etc/skillet/host`; they must not derive host identity
from the VM hostname or bake a host binary name into shared configuration.
Host-specific apply units and credential gates belong in host-specific Butane
includes. The host include also stages the host credential CLI; the shared
Butane compiler builds and stages both static binaries when they are not
provided. Remove the CLI's clamps-only delivery parser/guard, but reject
unsupported hosts and undeclared services before unlocking/mutating resources.
Do not pretend unfinished host templates are ready for VM provisioning.

Preserve deferred installation of both Caddy credentials followed by one
service start. Preserve recovery after partial delivery, idempotent Podman
secrets, tailnet access restrictions, persistent certificates, and secret-free
Caddyfiles. Use normal blocking systemctl starts, without `--wait`.

CLI target: `skillet secret deliver <host> caddy --environment production`
with the existing SSH and vault options. Production delivery uses the shared
host definition, environment config, credentials, and SSH transport. VM
provisioning reuses the Caddy payload and Cloudflare operations with its
recorded SSH transport and test environment.

Exit: rerunning unchanged delivery preserves container identity. A host with
Syncthing only has no Pi-hole route. Existing old `caddy_sites` payloads and
per-service hostname entries require explicit redelivery; old payloads are
rejected rather than silently reinterpreted. First production delivery migrates
an existing legacy token into the environment-scoped vault entry.

### 3. Shared Cloudflare token and DNS reconciliation (implemented; expand tests)

Follow [Cloudflare UI lifecycle](../design/cloudflare-ui-lifecycle.md). Do not
use manually created VM ACME tokens as an interim acceptance path.

Environment lookups use the DNS paths above. The configured Zone ID is fetched
exactly and the returned zone name must contain the UI domain before issuance
or record changes. The former zone-name entry is never treated as an ID.
The workstation issuer is an account-owned token with Account API Tokens Read
or Write and Zone Read on the selected zones. Skillet gets the account ID from
the zone response and uses account-token endpoints for permission discovery,
creation, listing, and revocation. The issuer needs no DNS Write.

Use `cloudflare-rs` for authenticated blocking HTTP transport and custom typed
JSON endpoint specs for token APIs and DNS comments not represented by its DNS
models. Do not use a Cloudflare CLI or shell program. Discover permission IDs
through the API; do not guess or bake IDs in.
Persistent token creation must use the existing atomic KDBX writer and conflict
protection. If saving fails after issuance, revoke the newly issued token or
retain recoverable ownership before returning an error. Never replace an
existing host token implicitly; rotation is explicit.

After Tailscale enrollment, reconcile one machine name `<host>.<ui-domain>`
with A and AAAA records for its verified Tailscale IPv4 and usable IPv6.
Report a missing family explicitly rather than creating an invalid record.
Create each `<service>.<host>.<ui-domain>` as a CNAME to the machine name;
create each caller-declared alias as a CNAME to its canonical service name.
Use short TTLs and `proxied=false` for every managed record. Zone DNS Write
covers A, AAAA, CNAME, and ACME TXT records with no extra permissions.
Reuse matching owned records; refuse unrelated conflicts, including existing
A/AAAA data at a desired CNAME name. Do not create a zone or configure
Tailscale split DNS. No deletion by name alone. Update machine addresses
without duplicating address records per UI. Reconcile removed aliases without
disrupting other routes or services; remove only owned obsolete records.

Separate credential lifetime from UI configuration: a persistent deployment
reuses its vault token; a disposable deployment records token ID, expiry,
record IDs, environment identity, and target ownership using existing run
metadata. Both use the same issuance, DNS reconciliation, and delivery functions.
No token value, domain value, or secret appears in routine logs or diagnostics.
Local ignored mode-0600 metadata may retain private configuration needed for
recovery, but never token values.

Journal external ownership immediately after each successful mutation. Use
deterministic ownership markers to recover a response-loss/crash window; refuse
ambiguous resources. Disposable cleanup removes only recorded/verified alias
CNAMEs, canonical CNAMEs, and then machine A/AAAA records before revoking its
token. If the token expired, mint and journal a temporary scoped
cleanup token through the issuer and revoke it afterward. Token expiry does
not delete DNS records. Do not sweep Caddy challenge TXT or unrelated records.
Failures retain enough state for retry and prevent discarding the run
directory. Persistent cleanup/rotation requires an explicit
operation and is never triggered by a VM disposal command.

Exit: new DNS paths and Zone ID validation are covered; issuance requires no
manually populated VM ACME entry; cleanup works after token expiry; both
environments use the same reconciler; a second apply creates no
duplicate token or records; failures at each external mutation can resume or
clean up. The retry tests inject API failures after token creation, between DNS
record creates, and during DNS deletion. Concurrent deployments claiming the same host/environment names
are refused. Do not append VM instance names to URLs silently; supporting
parallel instances requires separately selected base domains/environments.

### 4. Wire VM lifecycle and verify live ACME (wiring implemented; live check pending)

`test vm provision <host> <instance> --with-ui` explicitly opts into Cloudflare
and Caddy. Standard fixture smoke remains offline from Cloudflare. Provisioning
uses the test environment, journals disposable ownership, then creates the
token, records, delivers the config, and activates Caddy after Tailscale joins.
Destroy reconciles Cloudflare resources and Tailscale identity before local
ownership metadata is deleted. Missing vault entries name only required paths.

Reuse the retained VM for development, then run one fresh destroy/create/
provision cycle for final acceptance. User unlocks the KDBX in their own terminal;
subsequent commands reuse the three-hour cache. Do not print secret values.

Exit: staging DNS-01 succeeds, certificates have the derived service names,
and HTTPS reaches every enabled upstream through its canonical name and all
aliases from an authorized tailnet client. Verify the machine A/AAAA and UI
CNAME chains through the real client resolver, including IPv6 when available.
Staging certificates are untrusted: verify staging trust/identity explicitly;
do not claim trusted-browser acceptance or use unchecked `curl -k` as proof.
Verify actual client DNS resolution, denial outside tailnet, absence of direct
Syncthing GUI host publication, unchanged repeat apply, reboot persistence,
interrupted two-credential delivery recovery, and cleanup on destroy. Production
values and DNS records remain untouched. Verify renewal using a bounded explicit
scenario; if not exercised, record it as outstanding rather than passed.

## Required validation and documentation

Add meaningful separate test modules covering hostname/config validation,
single/multiple service and alias rendering, host/environment selection,
machine A/AAAA and CNAME reconciliation, alias removal, API ownership,
idempotency, cleanup, and interrupted issuance/delivery. Mock HTTP for CI so
routine CI needs no vault, Cloudflare token, or real DNS zone. Cover a second
host definition to catch clamps dependencies; do not require unfinished hosts
to boot before their templates exist.

Run workspace formatting, pedantic Clippy, tests, the workflow's integration
command, and affected real VM checks. Record exact commands/results and remaining
limits in `butane/ACCEPTANCE.md`. Update README command/vault setup instructions
and the three design documents as implemented behavior lands. Link this plan
from the migration plan; remove it only after preserving all remaining tasks.
Make focused commits only after required checks pass; do not install workstation
packages or push without the user's instruction.
