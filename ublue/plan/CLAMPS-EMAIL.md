# SMTP follow-ups

Native Postfix provisioning, encrypted structured credentials, isolated capture,
verified Mailjet STARTTLS/authentication and operator-confirmed Gmail receipt
have evidence in [acceptance](archive/ACCEPTANCE.md). Retained queue/map recovery
across boot/rebase is recorded separately. The implementation history is
[archived](archive/CLAMPS-EMAIL.md); commands are in the
[SMTP guide](../skillet/crates/smtp/README.md).

## Remaining

- Inspect delivered SPF/DKIM/DMARC headers and provider account limits; successful
  receipt alone does not establish all sender authentication or inbox placement.
- Exercise explicit invalid-certificate and authentication-failure cases using
  isolated disposable state. Keep routine tests off real mail delivery.
- Design restricted/authenticated bridge-client submission before exposing an
  endpoint; do not trust an entire bridge or publish an open LAN/tailnet relay.
- Add queue-age and delivery-failure monitoring through the module's declaration.

## Parked

UniFi SMTP/recovery-email integration is parked by operator decision and is not
an additional cutover gate. Resume application email configuration only when
requested. [Design](../design/email-delivery.md).
