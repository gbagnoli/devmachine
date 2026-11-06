# Public address DDNS

Retain the evaluated updater in a caller-selected rootful Quadlet, rather than
adding a second guest provider client. Installing Skillet or adding an environment
zone never enables DDNS. Private record leaves combine with reviewed public policy
through [configuration templates](configuration-templates.md).

DDNS owns only explicitly configured public-address records, disjoint from tailnet
UI records/CNAMEs. One production writer owns retained records; source/target
handover is explicit. Initial policy is A enabled, AAAA disabled, TTL 300 and no
unknown-record purging, preserving approved proxy status. Do not infer names from
UI namespaces or enable IPv6 without an explicit usable public-address policy.

The evaluated upstream 2.2.0 multi-platform digest is pinned. Its legacy JSON
loader reads /config.json as root and repeats at configured TTL. A root-only
Podman secret supplies the complete payload; environment-mode credentials would
bypass this schema. Its one-page 100-A-record limit is enforced by provisioning.
Digest pinning preserves that evaluated contract rather than registry auto-update.

The workstation resolves the environment Zone ID, issues a separate scoped child,
and persists it atomically. The master stays local. Versioned relative records
and takeover_existing=false reject conflicts and the UI namespace. Explicit
adoption permits only approved existing public records, never another owner's.
It does not stop a legacy writer by itself.

Production markers bind records to the caller. Disposable journals additionally
bind environment/instance, exact record names/IDs and bounded token ownership
before mutations. Repeat delivery reuses owned resources; cleanup validates and
removes only those resources. Ordinary smoke excludes the writer. Dedicated
credential phases preserve credential-free base/full operation; disk-protection
scope follows [storage](storage.md).

Commands: [DDNS](../skillet/crates/ddns/README.md).
Remaining rotation/cutover: [plan](../plan/CLAMPS-CLOUDFLARE-DDNS.md).
Source: [evaluated upstream](https://github.com/timothymiller/cloudflare-ddns/tree/7c6d5b43c1d400f4977f2b25bd44d784bb601ae3).
