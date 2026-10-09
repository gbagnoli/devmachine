# Fleet SMTP implementation

Status: native Postfix image packages and shared provisioning implemented.
Isolated native-consumer and rebuilt uCore capture acceptance passed.
Production provider/TLS authentication acceptance remains pending.
Mailjet entry fields, including sender, pass the production vault audit. Provider
authentication and verified sender/domain DNS remain live acceptance work.
See [the design](../design/email-delivery.md).

## Findings

- Chef configures SMTP directly for Wanderer and AdventureLog using application
  secrets. No fleet-wide Postfix/relay recipe was found. `secrets/` contains only
  its README locally, so the old provider cannot be identified from this checkout.
- `ucore-common` owns packages/static files; Skillet already owns credentials,
  canonical service composition, convergence and monitoring declarations.
- Operator identified Mailjet (`in-v3.mailjet.com:587`) and created one
  `skillet/smtp` entry with Username/Password and host/port/tls custom fields.
  Shared field-aware vault reading and required-field auditing are implemented;
  no provider authentication or outgoing email is performed by the audit.

## Ordered slices

1. **Partially complete:** Mailjet identified and the one-entry vault audit
   passed. Confirm the verified sender and domain, account limits and provider
   authentication during explicit live acceptance. Required STARTTLS on port
   587 is the selected policy. Prepare brief exact vault
   requirements through shared metadata/root checklist; keep provider password
   and private sender/endpoint leaves out of the public repo. Use a shared
   credential where its actual account scope allows it, with explicit test policy.
2. **Implemented:** add native Postfix and required authentication/TLS packages
   to the common image, initially inactive. Check the base image for existing packages and
   units first. Keep all runtime values and authentication material out of it.
3. **Implemented; capture VM gates passed:** add a cohesive `skillet_smtp` service
   crate and typed configuration template.
   Declare fleet inclusion centrally, separate from credential-free base apply.
   Deliver the entire prerequisite set before activation using existing encrypted
   credential workflow. Validate map backend, file permissions, SELinux and
   service/worker access using the actual runtime. Preserve queued mail across
   repeat apply, reboot and image rebase. Require authenticated verified-TLS
   upstream; no direct-MX fallback. Queue lives in `/var/spool/postfix`.
4. **Host/loopback implemented; application setup pending:** support host
   sendmail and loopback submission first. Add explicit bridge
   client access only for declared applications; do not publish LAN/tailnet SMTP
   or broadly allow arbitrary bridge clients. Applications own their SMTP client
   integration. Configure UniFi recovery email via a supported interface, with
   any necessary operator UI handoff recorded.
5. **Unit health implemented; queue/failure metrics pending:** add module-owned
   monitoring for service health, deferred queue age and send
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

## Next acceptance slice

The rebuilt image passed named disposable capture acceptance with enforcing
SELinux, loopback-only submission, boot map regeneration, and queue persistence
through reboot and signed image rebase. Record: `butane/ACCEPTANCE.md`.

Next: production-policy testing of certificate/authentication failures and
credential-map access under enforcing SELinux, then explicit Mailjet/Gmail
acceptance with provider-verified sender and SPF/DKIM/DMARC inspection. Configure
UniFi SMTP through its UI. Bridge client access and queue-age/send-failure
monitoring remain separate implementation work. No provider mail is sent by
routine smoke runs.
