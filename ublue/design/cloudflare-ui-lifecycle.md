# Cloudflare UI credentials and DNS lifecycle

The workstation account-owned issuer stays in KeePassXC. Zone Read resolves the
selected zone/account; account-token permissions create/list/revoke scoped child
credentials. Only child tokens reach hosts. Use cloudflare-rs blocking transport
with typed endpoint extensions for token APIs/comments absent from SDK models.

## Configuration and access

Each environment supplies a mandatory Zone ID and optional relative UI prefix,
defaulting to ui only when absent. Resolve the exact zone domain, append it to the
prefix and validate before mutation. Former full domains are not absolute-name
inputs. Production/test configuration is separate; the resolved namespace is
bound into ownership journals and used identically for delivery/cleanup.
[Setup](../skillet/crates/workstation/docs/cloudflare.md).

Machine A/AAAA use verified usable tailnet addresses; missing families fail
rather than fabricate values. Canonical service CNAMEs target the machine;
caller aliases target canonical UIs. Relative aliases stay within the selected
namespace and may contain a complete {host} label. A terminal {zone} placeholder
places an alias directly under the selected Cloudflare zone; cleanup validates
owned records against that zone while the journal retains the selected UI
namespace. Reject wildcard, duplicate,
cyclic and conflicting names before effects. Publish DNS-only records, never
LAN/public host addresses for private UIs.

Public callers use fixed zone-root labels independently of the private UI prefix.
An explicit public destination comes from the provisioning command, never from
the enrolled tailnet addresses. IP destinations use separate
`public.<host>.<zone>` A/AAAA records and service CNAMEs; an existing public
hostname can be the CNAME target instead. The same zone-scoped record owner,
journal, conflict checks and cleanup cover both address sets. Missing destinations
fail before mutations; there is no implicit public-IP discovery or NAT setup.

Every served alias gets certificate coverage and the same upstream/access policy;
a CNAME does not confer TLS identity. The versioned encrypted Caddy payload is
validated against the receiving caller's declared services/aliases.

Child Zone Read/DNS Write covers A/AAAA/CNAME/TXT across the entire selected zone,
not just a host's names. Separate children allow independent revocation; unique
names prevent collision, not zone-level access. Keep UI and DDNS ownership sets
disjoint. Public DNS names/addresses remain public even when source values are
excluded from the repository.

## Provision, reuse, and recovery

Persistent tokens are use-or-created through verified atomic KDBX persistence;
rotation is explicit. Failed saving revokes an issued token or retains sufficient
recovery ownership. Never silently replace a retained credential.

Disposable issuance journals complete host/environment/instance intent before
API mutation and exact token ID/expiry/owner afterward. Mode-0600 journals never
contain values. Deterministic names/markers support lost-response recovery;
ambiguity cannot justify deleting unrelated resources. Unrecoverable owned token
values need explicit revoke/reissue.

Reconciliation reuses matching owned records and journals IDs/type/value/marker;
unrelated conflicts fail. Namespace collisions are refused rather than silently
adding an instance to hostname conventions. Deliver both Caddy credentials
before activation. Runtime certificate state and repeat provisioning are retained.

## Disposal and expiry

Verify ownership before deleting alias/canonical CNAMEs and machine records;
never delete shared machine records still required by selected UIs. Revoke owned
disposable tokens, then clean Tailscale/local VM resources. Caddy owns its challenge
TXT lifecycle; never sweep other TXT records. Remove journals only after complete
cleanup, including partial/already-gone guests.

Expiry does not remove DNS. A short-lived journaled cleanup token can remove
verified records after the original token expires, then is revoked. Retained VMs
need explicit renewal for later ACME. Test disposal never removes production
credentials/records. Failure preserves evidence for retry.

Policy/recovery tests and live acceptance are tracked in
[the UI plan](../plan/GENERIC-PRIVATE-UIS.md). Related boundaries:
[secrets](secrets.md), [VM lifecycle](smoke-vms.md), [DDNS](cloudflare-ddns.md).
