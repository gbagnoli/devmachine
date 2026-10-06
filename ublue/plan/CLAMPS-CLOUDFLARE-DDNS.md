# Cloudflare DDNS port

Status: slices 1–3 and disposable test provisioning/cleanup implementation
complete, 2026-10-06. Shared DDNS crate, caller selection, separate apply
phase/unit, validated private config, token use-or-create, production encrypted
delivery, and journaled test-only DNS/token ownership are implemented. Routine
and fake-provider tests cover cleanup. Live DDNS DNS ownership acceptance is
parked because the test vault has no `ddns-config` entry. The disposable VM was
created, reached signed boot, partially provisioned, and destroyed with its
Tailscale identity removed; no DDNS token or DNS record was created. Syncthing
live acceptance remains parked and remote CI evidence is pending.

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

Implemented: upstream 2.2.0, index digest
`sha256:5f2471be9efd9f0c95f973645cc87f05d501020ded94d380a2120fbfdf812d3d`.
Actual image accepted a Podman secret at `/config.json`, mode `0400`, UID/GID 0,
with dummy credentials and no network. Schema v1 rejects malformed names,
duplicates and unsupported policies. Private live inventory remains pending.

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

Implemented: `ServiceConfig::Ddns` selects `skillet_ddns`; Clamps opts in.
`--phase ddns` and generic `skillet-ddns-apply.service` consume only
`cloudflare_ddns_config`. Full apply skips DDNS. Tests cover opt-in, missing
credentials/storage, root-only secret mount, no ports, no-op, rotation and retry.
Ordinary application acceptance excludes the separately provisioned DNS writer;
`acceptance_plan_with_ddns(true)` supplies its checks for the future live scenario.

- Add a DDNS capability and credential consumer to the canonical host profile;
  Clamps opts in, other callers may opt in. Keep declarations free of real DNS
  values. Service/UI/credential eligibility derives from that same declaration.
  The host binary explicitly selects the shared DDNS crate/service; having an
  environment zone in KeePassXC must never implicitly enable it. Reuse the
  existing environment zone entry, with private record names in `ddns-config`.
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

Production implemented: `secret deliver <host> ddns` uses the environment zone,
private `ddns-config` JSON and persistent `ddns-token`. Caddy and DDNS share the
atomic use-or-create helper. JSON may include `takeover_existing: true` to approve
adopting an existing public A record; UI namespace and other Skillet ownership
remain protected. New records receive a DDNS-specific ownership comment.
Fake-provider/transport tests cover token reuse, failed delivery retry, explicit
vault token rotation, failed saving/revocation and collisions. Test policy uses
an opt-in VM flow with bounded issuance and a mode-0600 ownership journal.

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

Implementation complete; live DNS acceptance remains parked. The disposable
provisioner persists VM/environment/zone identity, a unique test ownership
marker, exact configured record names, token identity and cleanup-token state
before provider mutation. It records created record IDs after verifying the
updater has published its marked A records. Disposal revalidates the journal
against the exact test VM and configured name allowlist, deletes only records
that still match the ownership marker and recorded IDs, then revokes the
bounded DDNS and cleanup tokens. A mismatch aborts cleanup without broadening
the deletion set.

- On a named VM verify a record receives the discovered public IPv4, stays
  DNS-policy compatible, leaves unrelated A/AAAA/CNAME/TXT records untouched,
  survives repeat apply/reboot, and reuses its credential. Test credential rotation
  and disposal without printing private values. Park live DDNS record checks
  until `skillet/environments/test/hosts/<host>/cloudflare/ddns-config` is
  present in KeePassXC. Record results in
  `../butane/ACCEPTANCE.md`.
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
