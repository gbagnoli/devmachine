# Fleet email delivery

Use native Postfix with a durable retry queue and standard host sendmail/loopback
submission. The image supplies Postfix/SASL dependencies but does not enable an
unconfigured relay. Shared caller composition selects the SMTP module; bootstrap
and ordinary full apply do not require provider credentials.

A container relay would need separate host submission integration; a lightweight
client alone does not provide this durable queue. Mailjet supplies authenticated
internet delivery, not an incoming mailbox or public MX service. A provider-verified
sender plus SPF/DKIM/DMARC is required independently of successful SMTP auth.

One structured vault entry supplies Username/Password and host/port/tls/sender
fields. Missing fields fail without printing values. Production requires verified
STARTTLS and never falls back to direct MX. Private sender/provider values use
normal encrypted delivery, never image layers.

Root/Postfix-readable RAM maps regenerate before startup, with explicit worker
permissions and Postfix configuration labels. Trust uses Fedora's extracted CA
bundle, not an assumed compatibility symlink. Package /var directories initialize
during convergence; image-layer contents do not initialize rebased hosts. Existing
queue contents are preserved and sender rewriting leaves recipients unchanged.

Submission listens only on loopback. Host-network apps can use it; bridge clients
need a separately restricted/authenticated endpoint. Unrelated containers must not
be implicitly trusted and no open LAN/tailnet relay is supplied. Application modules
own their clients and monitoring, not the core resource library.

Disposable policy uses an isolated capture sink with no provider credentials.
Explicit live-provider acceptance uses the same owned transport without changing
VM identity and sends no mail by itself. Persisted queue mode cannot switch
between capture/production during apply or boot preparation.

Setup/commands: [SMTP guide](../skillet/crates/smtp/README.md).
Enhancements/acceptance: [SMTP plan](../plan/CLAMPS-EMAIL.md).
References: [Postfix](https://www.postfix.org/SOHO_README.html),
[Gmail sender requirements](https://support.google.com/mail/answer/81126?hl=en).
