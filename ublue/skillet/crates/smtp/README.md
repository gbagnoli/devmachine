# Native SMTP relay

The shared fleet policy selects native Postfix. The uCore image supplies
`postfix` and `cyrus-sasl-plain`; activation requires delivered credentials.
Run Cargo commands from `ublue/skillet`.

## Setup

Create one KeePassXC entry `skillet/smtp`:

| Field | Value |
| --- | --- |
| Username | Mailjet SMTP username/API key |
| Password | Mailjet SMTP password/secret |
| `host` | `in-v3.mailjet.com` |
| `port` | `587` |
| `tls` | `starttls` |
| `sender` | Mailjet-verified sender email address |

Configure provider sender/domain verification and SPF/DKIM/DMARC. The
[vault audit](../workstation/README.md#vault-setup) checks required fields/formats,
not provider authentication or delivered message headers.

## Production

```sh
cargo run --release -p skillet -- secrets deliver HOST smtp --target USER@ADDRESS --activate
```

Inspect `skillet-smtp-apply.service`, `skillet-smtp-prepare.service` and
`postfix.service`. Host sendmail and loopback SMTP on port 25 submit mail;
Postfix authenticates upstream with required verified STARTTLS. A host-network
application may use `127.0.0.1:25`; bridge clients have no configured endpoint.
Do not expose port 25 on the LAN/tailnet or trust unrelated bridge containers.

Queued mail persists under `/var/spool/postfix`. Local senders are rewritten to
`sender`; recipients are preserved. The root/Postfix-readable provider map is
regenerated under `/run/postfix/skillet` from encrypted credentials, with SELinux
labeling and Fedora's extracted trust bundle. Do not print map contents.

## Disposable checks

Use an existing ready VM for isolated capture, without provider credentials:

```sh
cargo run --release -p skillet -- test vm provision HOST INSTANCE --smtp-only
```

Mail queues until a local sink listens on `127.0.0.1:1025`. Ordinary application
provisioning also selects capture. A retained relay cannot switch policy; use a
fresh instance for an explicitly authorized real-provider check:

```sh
cargo run --release -p skillet -- test vm create clamps mailjet --port 2202
cargo run --release -p skillet -- test vm provision clamps mailjet --smtp-only --live-smtp
```

This sends no message itself. Submit the intended message explicitly through the
owned VM SSH session, then dispose through the VM command.

`tests/native_acceptance.py BINARY HOST [--production-policy]` is an isolated
Podman systemd-container fixture requiring Postfix/Python and `--network none`.
It uses synthetic credentials, checks retry/queue/preparation, and in production
mode refuses a no-STARTTLS upstream. It refuses to run outside its test container.
Use Cargo's reported CLI artifact; this fixture does not manage VMs.

Design: [email delivery](../../../design/email-delivery.md).
Acceptance backlog: [SMTP plan](../../../plan/CLAMPS-EMAIL.md).
