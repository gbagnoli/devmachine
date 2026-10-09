# Fleet email delivery

Status: image packages and shared SMTP provisioning implemented. Native Postfix
container acceptance passed; rebuilt uCore VM and live provider acceptance remain
pending.

## Implementation boundary

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

## Provider entry

Mailjet is the existing provider. Read a single `skillet/smtp` entry:
Username and Password contain SMTP credentials; `host`, `port` and `tls` custom
fields hold the endpoint and security mode; `sender` holds the verified sender
address. Field-aware reading, auditing and encrypted delivery are implemented.
The audit requires these fields once for production fleet preparation. Test
provisioning neither reads nor delivers provider credentials. Only `starttls` is
accepted; production requires certificate and hostname verification.
Missing/invalid field names are public diagnostics; values are never printed. SMTP credentials are the provider’s API key/secret, rather
than the account login ([provider guide](https://documentation.mailjet.com/hc/en-us/articles/360043229473-How-can-I-configure-my-SMTP-parameters)).

## Delivery and access

Relay only through an authenticated external provider, using verified TLS;
never fall back to direct destination-MX delivery. Keep the queue in persistent
`/var/spool/postfix`, independent of the service-data mount. Shared templates
hold policy; private configuration and credentials use the existing vault and
systemd encrypted-delivery workflow. A root-owned, Postfix-group-readable
`texthash` map (`0640`, parent `0750`) under `/run/postfix/skillet` is regenerated
from `LoadCredentialEncrypted=` before Postfix starts. Native Fedora acceptance
verified the backend and permissions; enforcing SELinux remains a VM gate.
Sender canonical rewriting preserves recipients.
Never bake credentials or sender domains into an image.

Default listener: loopback only. UniFi uses host networking and can reach it.
Bridge containers need an explicitly declared internal endpoint with restricted
access, preferably authenticated submission; do not expose port 25 on the LAN
or tailnet, or trust an entire bridge containing unrelated containers by default.
Application modules own their SMTP client integration and monitoring declarations.
Postfix and credential-preparation unit health are declared by the SMTP module;
queue-age and delivery-failure metrics remain pending.

Production fleet hosts include SMTP as shared service policy. Test machines use
an isolated capture sink by default, with no external provider delivery or copied
production credentials. Real delivery is an explicit operator-controlled test.
Retained queues cannot switch between capture and production policy; apply and
credential preparation both enforce the persisted mode. The test relay targets
loopback port 1025; messages queue until an acceptance sink is running.

Use a provider-verified sender and SPF/DKIM/DMARC. The provider handles internet
SMTP delivery infrastructure and IP reputation; authentication does not guarantee
inbox placement. No incoming mailbox or public MX service is required.

References: [Postfix relay configuration](https://www.postfix.org/SOHO_README.html),
[Gmail sender requirements](https://support.google.com/mail/answer/81126?hl=en).

See [the implementation plan](../plan/CLAMPS-EMAIL.md).
