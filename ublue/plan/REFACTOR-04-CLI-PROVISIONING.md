# 4. Thin CLIs and shared workstation provisioning

Status: in progress; follows workstreams 1–3 in
[the prerequisite roadmap](SKILLET-REFACTOR.md).
Further feature milestones wait for that roadmap's completion.

## Read and locate

- Workstreams 1–3's VM transport, profiles, and guest input/resource contracts.
- `../skillet/crates/cli/src/{main,secret_delivery,cloudflare,tailscale}.rs`,
  `secret_delivery/vault_cache.rs`, and their tests.
- `../skillet/crates/cli-common/src/{lib,credential}.rs` and
  `../skillet/crates/core/src/credentials.rs`.
- `../design/{secrets,cloudflare-ui-lifecycle,smoke-vms,skillet-architecture}.md`.

Current defects: CLI modules combine parsing, vault persistence, transport,
remote credential installation, external APIs and lifecycle policy. Production
and VM delivery have duplicate implementations with different validation.

## Implementation sequence

1. Make binary modules responsible for parsing, tracing, interactive prompts,
   and dispatch. Move workstation logic into `skillet_workstation` with cohesive
   vault, providers, credential delivery and provisioning modules. Use typed
   inputs independent of Clap argument structures and `thiserror` library
   errors with preserved sources and sanitized diagnostics.
2. Keep guest apply/profile composition and encrypted credential installation
   in guest-facing libraries. Consolidate guest credential path/name validation,
   state inspection and atomic installation. A guest apply/credential path must
   not depend on a source checkout, workstation vault, or external API access.
   Preserve the generic CLI facade and thin host wrappers while documenting
   the actual dependency boundaries; do not duplicate the profile registry.
3. Encapsulate the existing KDBX reader/writer and kernel cache. Preserve XDG
   defaults, symlink handling, exact entry lookup, conflict checks, encrypted
   replacement verification, use-or-create safeguards, and the three-hour TTL.
   Provide testable unlock/cache boundaries without exposing master passwords
   through command arguments, logs, manifests or chat.
4. Use workstream 1's `GuestTransport` for production and disposable targets.
   Implement one credential-delivery operation taking host/profile, credential,
   consumer unit, activation policy and payload. Send payload only through stdin;
   verify host keys and use focused remote executable invocations with validated
   arguments. Keep multi-credential installation deferred until the set is
   complete. Remove both old parallel SSH/install implementations.
5. Model production/test differences as explicit policy: durable versus
   disposable values, tags, token lifetime, staging and cleanup ownership.
   Share environment lookup, zone/domain resolution, site/DNS derivation and
   delivery workflows. Preserve the current naming and conflict rules. Do not
   infer environment from a target name or expand credential scope.
6. Keep Cloudflare's official SDK transport and local HTTP fixture pattern.
   Replace Tailscale's curl HTTP adapter with a Rust HTTP client compatible with
   the build constraints, and inject its base endpoint for local protocol tests.
   Introduce narrow provider interfaces only for orchestration decisions that
   need fakes. Retain existing provider retry/ownership protections.
7. Connect the shared providers to workstream 1's lifecycle orchestrator.
   Persist pending enrollment/issuance and cleanup ownership before mutations;
   honor reuse/explicit rotation and partial credential-install recovery.
   Cleanup remains recoverable if the guest is absent. Remove API operations
   hidden in functions named for only one provider when they own multiple
   resources. Update usage and designs with completed behavior.

## Validation and exit criteria

- Local orchestration tests with dummy vaults and fake transports/providers
  cover declared/unsupported services, existing/missing secrets, vault conflict,
  partial delivery, deferred activation, API failure and cleanup retry. The
  selected policy, rather than host/environment branches, determines behavior.
- Local HTTP tests cover Tailscale auth-key/device-list/removal requests and
  failures, plus the existing Cloudflare regression cases. CI contacts no live
  provider and prompts for no private password.
- Verify three-hour expiry and cache clearing through the cache contract and,
  where available, a local dummy-key check; unavailable kernel checks are
  explicitly unverified. Do not test expiry by waiting three hours.
- Guest library dependency inspection and entry-point tests establish that
  guest apply/credential commands work without workstation state or checkout.
  Generic/per-host entry points retain the same behavior.
- On a named disposable VM verify delivery, repeat apply, rotation and recovery
  after deferred installation. Run enrollment/UI/disposal using the existing
  vault workflow when authorized credentials are available; preserve sanitized
  evidence and mark unavailable live checks honestly.
- Pass roadmap checks; no duplicated production/VM credential workflow remains,
  and runtime/CLI/library boundaries are reflected in designs and AGENTS.

## Progress

- Implemented: production and disposable-VM credential installations now use
  one `skillet_vm::credential::install` operation over `GuestTransport`. It
  accepts host, credential, consumer unit, activation policy, and payload as
  typed values; validates identifiers; sends secret bytes only on stdin; and
  reports sanitized guest errors. Both verify recorded SSH host keys.
- Implemented: Tailscale API calls use a bounded `reqwest` blocking client with
  an injectable base URL. Local HTTP tests cover OAuth auth-key creation,
  tagged device lookup/removal, and API failures. Tailscale no longer shells
  out to curl.
- Implemented: KeePassXC access and its named-session kernel password cache
  live in `skillet_workstation::vault`, outside Clap and guest apply code. The
  API encapsulates exact lookup, XDG path choice, symlink-safe atomic save,
  conflict detection, verified encrypted replacement, recovery backup, and
  three-hour cache/lock operations. It uses `thiserror` and does not implement
  `Debug` for the unlocked vault value.
- Implemented: the Cloudflare SDK client, token and DNS operations, domain
  validation, and local HTTP fixtures now live in `skillet_workstation`. Its
  library API uses `thiserror`; the CLI contains only the current orchestration
  decisions calling that API.
- Implemented: Tailscale's bounded HTTP client, OAuth auth-key creation,
  tagged-device lookup/removal, and local protocol tests now live in
  `skillet_workstation::tailscale`; the CLI no longer depends directly on
  `reqwest` for this provider.
- Implemented: `skillet_workstation::provisioning_policy` captures production
  versus test environment names and vault paths, ACME staging, Cloudflare
  credential lifetimes, cleanup-token lifetime, and Tailscale device tags.
  Delivery and cleanup consume this policy; the Cloudflare zone ID uses the
  shared `skillet/environments/dns/cloudflare-zone-id` vault entry.
- Implemented: production credential delivery and disposable-VM provisioning
  use `skillet_vm::SshTransport` with verified recorded host keys and literal
  executable arguments. Caddy activation, VM status probes, and the non-tailnet
  HTTP probe no longer build remote shell command strings. Both targets install
  credentials through the shared typed installer, with activation deferred
  until all related Caddy credentials are present.
- Implemented: Cloudflare ownership JSON, Tailscale enrollment markers, and
  enrolled-device records are persisted through
  `skillet_workstation::provisioning_state`. The library writes atomically with
  mode `0600`, rejects symlinked/non-regular state files, and binds a repeated
  enrollment marker to the same typed host/environment/instance identity. New
  journals retain those fields separately; old journal formats remain readable
  with exact expected-hostname/marker/token-name checks. No token value is
  persisted.
- Implemented: `skillet_workstation::ui_provisioning` derives Caddy sites and
  machine/service/alias DNS records from the canonical host profile, explicit
  environment policy, Cloudflare zone domain, optional relative UI prefix, and
  Tailscale addresses. Production delivery and disposable VM provisioning use
  this shared plan, removing duplicate derivation and host-specific UI logic.
- Implemented: the Tailscale provider parses guest status JSON and validates
  running state and IP addresses. VM orchestration now supplies the response
  bytes and handles retry timing; provider protocol interpretation and its
  malformed-response tests live with the Tailscale adapter.
- Implemented: bounded enrollment polling is also in the Tailscale provider.
  The CLI supplies a guest probe and explicit timeout/interval; retry behavior
  is covered with deterministic success, transient-error, and timeout tests.
- Implemented: a failed initial guest status command now aborts enrollment
  before issuing an auth key. Only a successful response with no addresses is
  treated as an unenrolled VM; transport and command failures remain errors.
- Implemented: `skillet_vm::credential::install_set` validates a complete,
  duplicate-free credential set before remote effects and applies one shared
  consumer/activation policy. Production and disposable Caddy provisioning use
  it to install both the sites payload and ACME token before activation.
- Implemented: disposable Tailscale enrollment sequencing now lives in
  `skillet_workstation::tailscale_enrollment`. The typed operation receives
  host, environment policy, VM instance/hostname, state directory, provider,
  and verified guest transport separately. It persists intent before effects,
  reuses an already-joined VM, installs a one-use key only after a successful
  empty status observation, verifies the tagged device, and records/removes
  recovery state. Tailscale cleanup remains in CLI.
- Implemented: disposable Cloudflare UI issuance and DNS reconciliation now
  live in `skillet_workstation::ui_provisioning`. The operation writes its
  owner journal before token creation, verifies the vault snapshot immediately
  before mutation, saves token ownership before DNS reconciliation, and records
  owned DNS IDs afterward. Fake-provider tests cover partial failure recovery.
- Pending: Cloudflare cleanup, guest delivery/activation and its retry policy,
  production plus named-VM acceptance, and the remaining environment/host
  delivery orchestration still span CLI and workstation modules. Complete the
  remaining sequence items before closing this workstream.
