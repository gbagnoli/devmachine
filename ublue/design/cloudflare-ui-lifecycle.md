# Cloudflare UI credentials and DNS lifecycle

Decision (planned): implement automated token issuance and cleanup before live
Caddy ACME acceptance. Do not require manually created smoke VM ACME tokens.
The workstation keeps `skillet/cloudflare/token-creator` in KeePassXC and mints
zone-scoped child tokens; only a child token is delivered to Caddy.

## Configuration and access

Each environment supplies `skillet/environments/<environment>/dns/ui-domain`
and `skillet/environments/<environment>/dns/cloudflare-zone-id` in the vault's
Password fields. The latter is a Cloudflare Zone ID. Resolve that exact ID
through the API and validate its returned zone name contains the UI domain
before mutations. Current code reads older paths and a zone name; migrating
those lookups is required, and a zone name must never be treated as an ID.
Production and test retain separate configuration.

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

Child tokens have Zone Read and DNS Edit restricted to the selected zone.
DNS Edit covers A, AAAA, CNAME, and ACME TXT records, so address publication
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

Status: design only. Implement and mock-test issuance, recovery, DNS ownership,
expiry cleanup, and disposal before a fresh staging ACME/HTTPS VM acceptance.
See [generic UI plan](../plan/GENERIC-PRIVATE-UIS.md),
[secret delivery](secrets.md), and [VM lifecycle](smoke-vms.md).
