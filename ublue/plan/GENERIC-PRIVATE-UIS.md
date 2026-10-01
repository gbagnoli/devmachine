# Generic private UI provisioning

Status: ready to implement, 2026-10-01. Planning only; existing Caddy code is
uncommitted and still restricts delivery and apply to clamps.

## Goal and fixed decisions

Every host declares its UI services. Shared provisioning derives each public
DNS name as `<service>.<host>.<ui-domain>` and delivers the configuration and
Cloudflare credential through the existing encrypted SSH delivery mechanism.
Syncthing is intended on every host; Pi-hole is included only where configured.
The caller selects the environment and declares the services. Caddy, DNS name
derivation, token issuance, and credential delivery must contain no clamps or
smoke special cases.

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
| `skillet/environments/<environment>/ui/domain` | Base UI domain |
| `skillet/environments/<environment>/cloudflare/zone-id` | Authorized existing zone |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` | Durable token for a persistent host |
| `skillet/cloudflare/token-creator` | Workstation-only token issuer |

Production and test are environment names, not distinct implementations.
Locate the existing domain entries privately before migration; reuse or move
their values, never invent or overwrite them. Read-only config migration
must not expose their values. Keep unrelated host secret paths unchanged.

The shared environment policy selects ACME staging for test and the normal
issuer for production. Token scope is Zone Read plus DNS Edit for the selected
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

### 1. Shared host UI declarations and environment configuration

Introduce a typed UI declaration containing a stable service name and its
bridge upstream (container DNS name and port). Each host caller supplies its
enabled services and Podman network through one reusable host definition.
Use that same declaration for Caddy and workstation DNS planning; do not
maintain two lists that can drift. Clamps declares Syncthing and Pi-hole;
other hosts can declare Syncthing without acquiring a Pi-hole dependency.
Declaring future services must not require editing Caddy's renderer.

Add a validated environment configuration and one hostname derivation helper.
Validate DNS labels, domain length, duplicates, empty service lists, and
upstream inputs before SSH delivery or remote API mutations. Reject schemes,
paths, ports, whitespace, and configuration injection in the base domain.
Validate the selected zone actually contains the base domain; never broaden
scope to another zone automatically.

Exit: a non-clamps host with only Syncthing derives exactly one route; the
same host definition works with either environment. Use reserved example
domains in tests. Missing config fails with the entry path, without values.

### 2. Generic Caddy configuration and delivery

Replace `CaddySites { pihole, syncthing, ... }` with a versioned service-list
payload and explicit issuer policy. Keep the host credential name `caddy_sites`
unless changing it is necessary. Derive hostnames on the workstation from
the shared declaration; validate the payload again on the receiving host.

Remove the clamps guard in Caddy apply. Pass the caller's network and services
into shared apply. Remove hardcoded `/var/usrlocal/bin/skillet-clamps` from
the delivery helpers; resolve the target binary from the validated host
definition. Remove the CLI's clamps-only delivery parser/guard, but reject
unsupported hosts and undeclared services before unlocking/mutating resources.
Do not pretend unfinished host templates are ready for VM provisioning.

Preserve deferred installation of both Caddy credentials followed by one
service start. Preserve recovery after partial delivery, idempotent Podman
secrets, tailnet access restrictions, persistent certificates, and secret-free
Caddyfiles. Use normal blocking systemctl starts, without `--wait`.

CLI target: `skillet secret deliver <host> caddy --environment production`
with the existing SSH and vault options. The shared delivery function accepts
host definition, environment config, credentials, and transport; VM provisioning
calls this same function with its recorded transport and test environment.

Exit: rerunning unchanged delivery preserves container identity. A host with
Syncthing only has no Pi-hole route. Existing old `caddy_sites` credentials
receive a documented explicit redelivery/migration path; do not silently
interpret the old payload as the new schema.

### 3. Shared Cloudflare token and DNS reconciliation

Add a Rust HTTP client module with typed errors and injectable API boundary.
Reuse project HTTP dependencies; do not use a Cloudflare CLI or shell program.
Discover required permission IDs through the API; do not guess or bake IDs in.
Persistent token creation must use the existing atomic KDBX writer and conflict
protection. If saving fails after issuance, revoke the newly issued token or
retain recoverable ownership before returning an error. Never replace an
existing host token implicitly; rotation is explicit.

After Tailscale enrollment, reconcile DNS-only A/AAAA records for every declared
UI to the target's actual tailnet addresses. Reuse matching owned records;
refuse unrelated conflicting records. Do not create a new zone or configure
Tailscale split DNS. No record deletion by name alone.

Separate credential lifetime from UI configuration: a persistent deployment
reuses its vault token; a disposable deployment records token ID, expiry,
record IDs, environment identity, and target ownership using existing run
metadata. Both use the same issuance, DNS reconciliation, and delivery functions.
No token value, domain value, or secret appears in routine logs or diagnostics.
Local ignored mode-0600 metadata may retain private configuration needed for
recovery, but never token values.

Journal external ownership immediately after each successful mutation. Use
deterministic ownership markers to recover a response-loss/crash window; refuse
ambiguous resources. Disposable cleanup removes only recorded/verified records
and revokes only its token. Failures retain enough state for retry and prevent
discarding the run directory. Persistent cleanup/rotation requires an explicit
operation and is never triggered by a VM disposal command.

Exit: both environments use the same reconciler; a second apply creates no
duplicate token or records; failures at each external mutation can resume or
clean up. Concurrent deployments claiming the same host/environment names
are refused. Do not append VM instance names to URLs silently; supporting
parallel instances requires separately selected base domains/environments.

### 4. Wire VM lifecycle and verify live ACME

Extend `test vm provision <host> <instance>` with explicit live-UI opt-in,
for example `--with-ui`. Standard fixture smoke remains offline from Cloudflare.
The caller supplies test environment and disposable ownership; the shared UI
pipeline handles token, records, config delivery, and Caddy activation after
Tailscale is connected. Destroy cleans Cloudflare resources and Tailscale
identity before deleting local ownership metadata. Missing vault entries fail
before external mutations, naming only the required paths.

Reuse the retained VM for development, then run one fresh destroy/create/
provision cycle for final acceptance. User unlocks the KDBX in their own terminal;
subsequent commands reuse the three-hour cache. Do not print secret values.

Exit: staging DNS-01 succeeds, certificates have the derived service names,
and HTTPS reaches both enabled upstreams from an authorized tailnet client.
Staging certificates are untrusted: verify staging trust/identity explicitly;
do not claim trusted-browser acceptance or use unchecked `curl -k` as proof.
Verify actual client DNS resolution, denial outside tailnet, absence of direct
Syncthing GUI host publication, unchanged repeat apply, reboot persistence,
interrupted two-credential delivery recovery, and cleanup on destroy. Production
values and DNS records remain untouched. Verify renewal using a bounded explicit
scenario; if not exercised, record it as outstanding rather than passed.

## Required validation and documentation

Add meaningful separate test modules covering hostname/config validation,
single/multiple service rendering, host/environment selection, API ownership,
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
