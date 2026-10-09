# Fleet email delivery

Status: proposed after repository/provider research; not implemented.

## Recommended boundary

Use native Postfix as a submission relay with a persistent retry queue. Install
Postfix and its required SASL/TLS support in `ucore-common`; keep the shipped
service inactive until configuration and credentials are delivered. A shared
`skillet_smtp` service module owns runtime convergence, selected from the
canonical fleet host declarations. Do not put SMTP policy in `skillet_core` or
make the credential-free base/bootstrap phase require provider access.

This gives host programs the standard sendmail interface and host-network
applications a loopback SMTP endpoint. The packages are static OS dependencies;
provider endpoints, private sender identities and credentials are runtime inputs.
A container relay is possible, but would additionally need host submission
integration. A lightweight client alone does not supply the required durable
queue without extra machinery.

## Delivery and access

Relay only through an authenticated external provider, using verified TLS;
never fall back to direct destination-MX delivery. Keep the queue in persistent
`/var/spool/postfix`, independent of the service-data mount. Shared templates
hold policy; private configuration and credentials use the existing vault and
systemd encrypted-delivery workflow. Validate credential-map support, service
permissions and worker/chroot behavior before selecting its concrete backend.
Never bake credentials or sender domains into an image.

Default listener: loopback only. UniFi uses host networking and can reach it.
Bridge containers need an explicitly declared internal endpoint with restricted
access, preferably authenticated submission; do not expose port 25 on the LAN
or tailnet, or trust an entire bridge containing unrelated containers by default.
Application modules own their SMTP client integration and monitoring declarations.

Production fleet hosts include SMTP as shared service policy. Test machines use
an isolated capture sink by default, with no external provider delivery or copied
production credentials. Real delivery is an explicit operator-controlled test.
Do not send queued lab mail later when changing a test instance to another policy.

Use a provider-verified sender and SPF/DKIM/DMARC. The provider handles internet
SMTP delivery infrastructure and IP reputation; authentication does not guarantee
inbox placement. No incoming mailbox or public MX service is required.

References: [Postfix relay configuration](https://www.postfix.org/SOHO_README.html),
[Gmail sender requirements](https://support.google.com/mail/answer/81126?hl=en).

See [the implementation plan](../plan/CLAMPS-EMAIL.md).
