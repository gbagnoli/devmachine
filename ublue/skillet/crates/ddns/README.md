# Cloudflare DDNS

The caller must declare DDNS. Run Cargo commands from `ublue/skillet`.

## Setup

Complete [Cloudflare issuer setup](../workstation/docs/cloudflare.md). In Password:

- `skillet/environments/prod/dns/cloudflare-zone-id`: selected environment Zone ID.
- `skillet/environments/prod/hosts/<host>/cloudflare/ddns-dns-name`: approved
  relative DNS record name, without the zone suffix.

Public policy is in the DDNS template in
[configuration-templates.json](../workstation/src/configuration-templates.json).
Private names use typed `$secret` references. A host override replaces the whole
shared template. Do not store a JSON configuration document in the vault.

## Provision

Stop the prior writer and approve the selected record set first:

```sh
cargo run --release -p skillet -- secrets deliver HOST ddns --target USER@ADDRESS --activate
```

Skillet creates/reuses
`skillet/environments/prod/hosts/<host>/cloudflare/ddns-token`; close KeePassXC
before saves. Set `takeover_existing=true` in a reviewed template only to adopt
an existing public A record. Conflicts and another Skillet owner are refused.
Edit the token deliberately and redeliver for rotation. On the guest,
`systemctl start skillet-ddns-apply.service` reuses encrypted input. Inspect
`cloudflare-ddns.service` and its journal for updates.

For an owned VM, set its test record-name entry and use
`test vm provision HOST INSTANCE --with-ddns`. Destroy cleans its journaled
records/tokens. Do not use direct production delivery for test policy.

Designs: [DDNS ownership](../../../design/cloudflare-ddns.md),
[template format](../../../design/configuration-templates.md).
Acceptance backlog: [DDNS plan](../../../plan/CLAMPS-CLOUDFLARE-DDNS.md).
