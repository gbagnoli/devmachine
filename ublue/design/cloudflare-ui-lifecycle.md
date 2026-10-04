# Cloudflare UI credentials and DNS lifecycle

Decision: the workstation issuer creates scoped Cloudflare child tokens and
reconciles owned DNS before live Caddy ACME acceptance. Do not require manually
created smoke VM ACME tokens.
The workstation keeps `skillet/cloudflare/token-creator` in KeePassXC and mints
zone-scoped child tokens through Cloudflare's account-token API; the account ID
comes from the selected zone response. Only a child token is delivered to Caddy.
The issuer is an account-owned API token with Account API Tokens Read or Write
and Zone Read on the configured zones. Zone Read allows preflight validation to
fetch the selected zone and its account ID. Skillet uses account-token
permission discovery, creation, listing, and revocation endpoints; DNS Write
remains confined to child tokens.

## Configuration and access

Each environment requires `skillet/environments/<environment>/dns/cloudflare-zone-id`
in the vault's Password field. Fetch that exact zone's domain from Cloudflare.
The optional `skillet/environments/<environment>/dns/ui-domain` is a relative
prefix, defaulting to `ui` only when absent. Always append the zone's domain:
`ui.whatever` in zone `example.com` resolves to `ui.whatever.example.com`.
This removes redundant full-domain configuration and keeps namespaces within
the selected zone. Empty or invalid prefixes fail before issuance or DNS
mutations. Former full-domain values require a deliberate vault migration;
they are never interpreted as absolute names. Production and test retain
separate configuration. Delivery and cleanup use the same resolution rule;
ignored ownership metadata retains the resolved full namespace for verification.

Host declarations derive a machine name `<host>.<ui-domain>` with one A
record for its verified Tailscale IPv4 address and one AAAA for its verified,
usable Tailscale IPv6 address. A missing address family is reported explicitly;
do not invent an address or publish an unusable AAAA record. Each canonical UI
name `<service>.<host>.<ui-domain>` is a CNAME to that machine name. Keep all
records DNS-only with a short TTL. Address changes update the machine records
without rewriting every UI record. DNS-01 can issue certificates before
address publication, but working UI URLs require this record chain.

Each host caller may declare zero or more aliases per UI in the same service
declaration that supplies its upstream. Aliases are domain-free relative DNS
names beneath the selected environment's UI domain, optionally containing an
explicit `{host}` label placeholder. This keeps real domains in KeePassXC
and gives test and production the same declaration. Absolute names and aliases
outside this namespace require a later explicit vault-backed design; never
broaden token scope automatically. Validate expanded names and reject wildcard,
cyclic, conflicting, or duplicate names across services before mutations.

Each alias is a CNAME to its canonical UI name. Caddy serves the canonical
name and every alias through the same upstream and tailnet restrictions;
aliases are additional routes, not redirects. Caddy must obtain and renew
certificate coverage for every served name, whether certificates are separate
or contain multiple names. A CNAME does not give its source name the target's
TLS identity. Include the alias set in the versioned encrypted Caddy payload
and validate it against the receiving host's declaration. Verify DNS, TLS
hostname identity, and upstream responses for canonical names and aliases.
Publish no LAN or public host addresses.

Child tokens have Zone Read and DNS Write restricted to the selected zone.
DNS Write covers A, AAAA, CNAME, and ACME TXT records, so address publication
needs no additional permission. The scope covers the whole zone, not an individual
host's records. Separate tokens allow independent revocation, while the issuer
stays on the workstation. Public DNS exposes the published names and tailnet
addresses; keep those values out of the public repository.

## Provision, reuse, and recovery

Persistent deployments use-or-create a token at
`skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` through
the atomic KDBX writer. Reuse existing tokens; rotation is explicit. If saving
an issued token fails, revoke it or retain ownership sufficient for recovery.
Never silently replace an existing credential.

Disposable deployments mint a bounded-lifetime token for the recorded VM
owner, using no manually populated ACME entry. Journal token ID, expiry,
environment, and target ownership in ignored mode-0600 run metadata. Keep the
token value only in memory during delivery and in the existing encrypted host
credential and Podman secret path, never in metadata or logs. Record issuance
intent before API calls and ownership immediately afterward. Deterministic
ownership markers must allow recovery after lost responses; ambiguity fails
without deleting resources. If a token value cannot be recovered after an
interruption, revoke the owned token and issue a replacement explicitly.

Reconcile the machine A/AAAA records, canonical UI CNAMEs, and all alias
CNAMEs with the enrolled device and host declarations. Reuse matching owned
records, reject unrelated conflicts, and journal record IDs and expected type/value/ownership. Concurrent instances
claiming the same environment/host names must be refused; never append an
instance to a UI hostname silently. Install both Caddy credentials before
activation. Repeat provisioning reuses tokens, records, and certificate state.

## Disposal and expiry

Delete verified owned alias CNAMEs first, then canonical UI CNAMEs, then
machine A/AAAA records, and revoke the disposable token. Removing an alias
from a host declaration reconciles its owned DNS record and Caddy route; do
not delete shared machine records while other declared UIs still use them.
Caddy manages its own challenge TXT lifecycle;
never sweep other TXT records or delete records solely by hostname. Clean up
the Tailscale device and local VM only after external ownership is resolved.
Failures retain metadata for retry, including partial or already-gone guests.

Token expiry does not remove DNS records. If the VM token is expired or
revoked, mint and journal a temporary zone-scoped cleanup token using the
workstation issuer, delete the verified records, and revoke the cleanup token.
Retained VMs need explicit token renewal for later ACME operations. Production
credentials and records are never disposed by test VM commands.

Implementation status: `cloudflare-rs` supplies authenticated blocking HTTP
transport; custom typed JSON endpoints use that transport for token APIs and
DNS record comments that the crate's DNS models do not expose. Persistent
delivery use-or-creates and saves its scoped token in KeePassXC. The
workstation UI provisioner now persists disposable owner intent, issues and
journals its scoped token, reconciles DNS, records owned record IDs, and owns
disposable cleanup. Persistent delivery also owns tailnet device lookup,
ACME-token reuse or issuance, migration of the legacy production entry,
DNS reconciliation, complete credential-set delivery, and deferred Caddy
activation. Cleanup validates the recorded identity and current
environment configuration before mutation, removes marker-owned DNS, revokes
the disposable and temporary cleanup tokens, and removes its journal last.
Failures retain the journal for retry. Disposable guest delivery now installs
both credentials before Caddy activation, checks the explicit non-tailnet
denial response for every canonical UI and alias, and only then revokes older
tokens with the same deterministic name. Local
tests cover the SDK transport, permission group
discovery, scoped token payloads, address-family validation, and DNS planning.
Live 2026-10-02 staging acceptance issued certificates for each
declared canonical UI and alias, verified DNS and HTTPS over IPv4/IPv6, and
disposed the VM, DNS records, and child tokens. CI tests inject an ambiguous
token-create response, a DNS-create failure after a partial reconciliation,
and a DNS-delete failure during cleanup; each retry converges without
duplicates or deleting unrelated records. Opt-in smoke provisioning also
requests each configured UI from the guest's loopback source and requires
Caddy's explicit 403 response. This verifies the source-address matcher for a
non-tailnet source; denial from a separate external network and production
certificate acceptance remain pending.

See [generic UI plan](../plan/GENERIC-PRIVATE-UIS.md),
[secret delivery](secrets.md), and [VM lifecycle](smoke-vms.md).
