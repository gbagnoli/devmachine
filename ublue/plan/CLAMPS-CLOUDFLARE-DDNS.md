# Cloudflare DDNS port

Status: planned, 2026-10-06. Next missing Clamps service from the
[parity audit](CLAMPS-PARITY.md). No DDNS service, vault schema, or delivery command
described below is implemented yet. Refactor implementation is committed;
Syncthing live acceptance is parked and remote CI evidence is pending. Planning
can proceed; keep unavailable runtime acceptance explicit during implementation.

Read `../AGENTS.md`, `../skillet/AGENTS.md`, `../design/cloudflare-ddns.md`,
`../design/secrets.md`, `../design/cloudflare-ui-lifecycle.md`, and
`../design/smoke-vms.md`. Install nothing on the workstation. Use Rust
resources/crates for orchestration and provider APIs; never embed shell programs.

## Source contract

`site-cookbooks/rupik/recipes/cloudflare_ddns.rb` is included by Rupik's default
recipe. It supplies one Cloudflare zone and private subdomain configuration;
A is enabled, AAAA disabled, TTL 300, and `purgeUnknownRecords=false`.
`site-cookbooks/cloudflare_ddns/recipes/default.rb` runs the image family
`docker.io/timothyjmiller/cloudflare-ddns` with a JSON config at `/config.json`.
It also creates a host account and sets PUID/PGID environment variables. Do not
assume these variables control the process or reproduce unnecessary host accounts.

The live image version, polling cadence, full record/proxy configuration, and
any second updater on Rupik need private inventory before production activation.
The legacy `rupik::network` cron is not selected by the default include path.

## 1. Establish the private config and image contract

- Inspect official image source/docs for the selected version, JSON schema,
  config-file access, refresh cadence, runtime identity, and credential support.
  Pin an evaluated image version/digest in the implementation; do not invent its
  behavior from the Chef variables. Record findings in the design.
- Initial support is one selected environment zone, obtained from existing
  `skillet/environments/<environment>/dns/cloudflare-zone-id`. Verify the retained
  DDNS records actually belong to it; an additional zone needs an explicit schema
  extension, never automatic token-scope broadening.
- Planned host config entry:
  `skillet/environments/<environment>/hosts/<host>/cloudflare/ddns-config`.
  Define a versioned JSON schema with relative record names and explicit approved
  proxy state; initial address/TTL/purge policy matches Chef. Resolve record names
  directly below the selected zone, independently of `dns/ui-domain`.
- Validate names, duplicate/conflicting records, and overlap with the host's UI
  records before token issuance or delivery. Existing unrelated records require
  an explicit takeover decision. Do not populate production values in this repo.

Exit: documented image/config contract and pure validation/rendering tests using
reserved example names. Secret-file consumption is confirmed before activation
work. Private source inventory remains explicit if the host is unavailable.

## 2. Declare the service and compose its Quadlet

- Add a DDNS capability and credential consumer to the canonical host profile;
  Clamps opts in, other callers may opt in. Keep declarations free of real DNS
  values. Service/UI/credential eligibility derives from that same declaration.
- Add an independently credential-gated DDNS apply phase/unit, following the
  existing Caddy separation. Base/full convergence must continue to work before
  DDNS credentials exist; declaring the service must not add its credential to
  the ordinary full-apply prerequisites. Keep this dispatch shared across hosts.
- Add the small service module using existing Podman and convergence resources.
  Require the shared mounted service-data/Podman store. Use a rootful container,
  restart/boot policy, network-online ordering, and the evaluated image.
  Avoid a private-UI declaration or host port publications.
- Planned credential/Podman secret name: `cloudflare_ddns_config`. Parse and
  validate the delivered payload before installing the secret. Mount the secret
  at `/config.json` with the numeric UID/GID/mode proved by the image contract;
  explicitly model process identity and namespace mapping if needed.
- If the image cannot consume this secret file, stop this slice and document a
  revised integration rather than writing token-bearing config to disk silently.
- Connect changed config/secret revisions to the existing restart/retry behavior.
  Unchanged convergence must preserve container identity. Missing credentials must
  leave DDNS inactive without breaking unrelated service apply.

Exit: CI covers caller opt-in, required credentials, secret mount, runtime identity,
missing data mount, no-op repeat apply, rotation, and failed-activation retry.
Routine CI requires no vault, external IP discovery, Cloudflare, or hypervisor.

## 3. Issue and deliver the DDNS credential

- Reuse the workstation Cloudflare SDK/account-token issuer and atomic KDBX writer.
  Planned token path:
  `skillet/environments/<environment>/hosts/<host>/cloudflare/ddns-token`.
  Persistent policy uses-or-creates a separate zone-scoped Zone Read/DNS Write
  token; it never delivers the token creator or reuses the Caddy token implicitly.
- Build the image-specific JSON in memory from validated config and child token.
  Deliver through existing encrypted SSH/systemd credential handling, then create
  the Podman secret. Never put the token or complete payload in argv, logs,
  manifests, recordings, or a plaintext host config file.
- Deliver all credentials needed by the selected activation phase before starting
  consumers. Preserve existing commands; add shared eligibility/phase support as
  necessary rather than a Clamps-specific command path.
- Use the existing issuance journals, verified transport, run lock, immutable
  target checks, and retry policy. Rotation is explicit; repeated delivery reuses
  existing persistent credentials. Keep provider dependencies workstation-side.

Exit: fake-provider tests cover use-or-create, secret-safe errors, interrupted
delivery/activation, and rotation. No guest apply depends on KeePassXC or the
workstation issuer.

## 4. Disposable acceptance and production handover

- Make test DDNS activation opt-in. Use only explicitly configured test record
  names and the test environment zone; refuse production config on a disposable
  VM. Journal ownership before writes, detect record collisions, and use a
  bounded-lifetime token. Zone tokens cannot enforce per-record scope; allowlisting
  and ownership validation belong to provisioning and the updater's config.
- Keep DDNS record ownership separate from the UI lifecycle. The workstation may
  preflight/journal test records, but the DDNS runtime is their continuing writer.
  Cleanup verifies recorded identity and records, then deletes only test-owned
  records and revokes the DDNS token. Persistent disposal never deletes production
  records. If the image cannot preserve reliable ownership metadata, design the
  record-journaling interface before permitting test record creation.
- On a named VM verify a record receives the discovered public IPv4, stays
  DNS-policy compatible, leaves unrelated A/AAAA/CNAME/TXT records untouched,
  survives repeat apply/reboot, and reuses its credential. Test credential rotation
  and disposal without printing private values. Park these checks while Flatpak
  libvirt is unavailable; record results in `../butane/ACCEPTANCE.md` later.
- Before production: privately inventory Rupik's exact records, TTL/proxy state,
  image/version/cadence, and active updater services/cron. Approve the takeover
  record set; stop the old writer, activate Clamps, verify records and services,
  and keep a rollback path that runs only one writer. Do not trigger this handover
  as part of an implementation test.

Exit: local checks pass and named-VM results or exact unavailable checks are
recorded. Production mutation and DNS-writer handover remain a separate cutover.

## Handoff

Implement slices in order with focused commits. Update this plan/design and the
parity checklist after each slice; do not claim production parity from rendering
tests. Datadog port and UniFi backup/restore are the following work items.
