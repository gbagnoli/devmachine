# Fleet SMTP implementation

Status: investigation completed; implementation pending. UniFi isolated restore
acceptance is complete. Native Postfix + shared Skillet module is the recommended
approach; confirm the existing provider before selecting credentials and endpoints.
See [the design](../design/email-delivery.md).

## Findings

- Chef configures SMTP directly for Wanderer and AdventureLog using application
  secrets. No fleet-wide Postfix/relay recipe was found. `secrets/` contains only
  its README locally, so the old provider cannot be identified from this checkout.
- `ucore-common` owns packages/static files; Skillet already owns credentials,
  canonical service composition, convergence and monitoring declarations.
- No credentials were read or new vault requirements introduced by this research.
  Do not assume the unlocked vault contains the old provider configuration.

## Ordered slices

1. Confirm the operator’s existing provider and verified sender. Check SMTP
   authentication, required TLS/port, sending limits and domain verification
   against that provider’s official documentation. Prepare brief exact vault
   requirements through shared metadata/root checklist; keep provider password
   and private sender/endpoint leaves out of the public repo. Use a shared
   credential where its actual account scope allows it, with explicit test policy.
2. Add native Postfix and required authentication/TLS packages to the common
   image, initially inactive. Check the base image for existing packages and
   units first. Keep all runtime values and authentication material out of it.
3. Add a cohesive `skillet_smtp` service crate and typed configuration template.
   Declare fleet inclusion centrally, separate from credential-free base apply.
   Deliver the entire prerequisite set before activation using existing encrypted
   credential workflow. Validate map backend, file permissions, SELinux and
   service/worker access using the actual runtime. Preserve queued mail across
   repeat apply, reboot and image rebase. Require authenticated verified-TLS
   upstream; no direct-MX fallback. Queue lives in `/var/spool/postfix`.
4. Support host sendmail and loopback submission first. Add explicit bridge
   client access only for declared applications; do not publish LAN/tailnet SMTP
   or broadly allow arbitrary bridge clients. Applications own their SMTP client
   integration. Configure UniFi recovery email via a supported interface, with
   any necessary operator UI handoff recorded.
5. Add module-owned monitoring for service health, deferred queue age and send
   failures without credential/message-body disclosure. Test hosts use a local
   capture sink and environment tags; provider delivery remains explicit opt-in.

## Exit criteria and acceptance

- Routine CI needs no vault/provider: typed rendering, missing/invalid inputs,
  prerequisite activation, repeat apply, metadata preservation and recipient/access
  restrictions tested through injected boundaries.
- Named disposable VM: host sendmail and intended SMTP clients reach the sink;
  unauthorized clients cannot relay; TLS certificate/authentication failures do
  not trigger plaintext delivery. Restart/reboot/rebase preserve queued messages
  and retry after a simulated upstream outage without regenerating credentials.
- Explicit live acceptance after provider setup: send a message to an
  operator-controlled Gmail inbox. Inspect SPF/DKIM/DMARC results and actual
  delivery; no guaranteed inbox placement claim. Verify UniFi recovery email.
  Do not send during routine smoke runs.
- Record exact passed/pending checks in `butane/ACCEPTANCE.md`, maintain designs,
  README usage and the root secrets checklist; focused validated commits, no push.

References: [Postfix relay configuration](https://www.postfix.org/SOHO_README.html),
[Gmail requirements](https://support.google.com/mail/answer/81126?hl=en).
