# 4. Thin CLIs and shared workstation provisioning

Status: planned; follows workstreams 1–3 in
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
