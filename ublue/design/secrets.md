# Secret storage and delivery

Use KeePassXC KDBX as the portable authoritative store, accessed directly through
workstation Rust. Syncthing moves the encrypted database without introducing a
custom backup/teleport format. XDG selects its default location. A terminal-only
unlock is cached in the Linux kernel keyring for three hours with expiry; no
plaintext password file or environment-variable password is used. Missing stores
fail before prompting. [Operating setup](../skillet/crates/workstation/README.md).

## Ownership and persistence

Paths separate generic credentials, host values, environment values and owned
VM-instance archives. The shared [requirements catalog](../skillet/crates/workstation/src/secret-requirements.json)
is the source for audit and human setup documentation. Optional host capabilities
must be caller-selected; credential presence never enables them. Structured
entries declare required standard/custom fields and validators. Ordinary clear
policy stays in [source templates](configuration-templates.md), with private leaves
resolved at the workstation boundary.

Use-or-create applies only to fresh state. A missing entry beside an existing
host secret requires recovery, not invention of a replacement. Atomic KDBX writes
verify encrypted readback, detect concurrent modification and preserve unrelated
entries, attachments and history. Synced vault conflicts require explicit repair.
Generated values/rotations are never implicit resets of retained credentials.

## Delivery and consumption

After signed bootstrap, verified SSH stdin carries only the selected payload.
The host Rust installer encrypts with systemd host-key encryption, verifies the
staged encrypted file and atomically replaces root-only credentials under
`/etc/credstore.encrypted/skillet`. Current production and test delivery use that
host key; dedicated systemd TPM binding is not claimed. Encrypted-root policy
protects persistent runtime copies separately; see [storage](storage.md).

Systemd LoadCredentialEncrypted supplies memory-backed inputs to the receiving
phase. Podman secrets are created through stdin and Quadlets refer to names with
Secret directives, not literal values in Environment. Podman's disk copy is not
protected by systemd credential encryption itself. Values never enter argv,
recordings, manifests or public image layers.

Runtime configuration remains mutable; do not commit a provisioned OCI image
containing host state or secrets. Static OS dependencies belong to the public
image layer, while Skillet owns dynamic convergence.

Staging is separate from activation. Related credential sets are complete before
starting their consumer. Narrow delivery defaults to defer; explicit remote
[host application](host-application.md) owns phase ordering/retries. Provider
mutations may occur during Caddy/DDNS staging.

## Provider and native consumers

Tailscale OAuth and Cloudflare issuers stay on the workstation. Hosts receive
one-use enrollment/scoped child tokens only. Disposable identities and provider
journals belong to the [VM lifecycle](smoke-vms.md); environment and instance are
not inferred from runtime hostnames. [Cloudflare lifecycle](cloudflare-ui-lifecycle.md)
owns token expiry, DNS ownership and cleanup policy.

Native SMTP and networking receive typed structured payloads over the same
channel. Sensitive consumer files regenerate in restricted /run paths before
their services start. SMTP test capture skips provider credentials; physical
[networking](host-networking.md) delivery skips disposable policy entirely.

## Root recovery archives

An independent passphrase and volume-bound protected `root-luks.header`
attachment live in the portable vault. Persist pending state before enrollment;
verify against the intended volume before marking complete. Reuse pending keys,
reject a different volume and never silently replace verified recovery material.
Header refresh preserves the passphrase and verifies the persisted bytes.

Replacement removes the current named attachment reference before insertion,
preserving dense binary IDs required by the KDBX writer. Shared/history references
that cannot be safely detached cause refusal, not data loss. Historical headers
retain old keyslots, so encrypted vault backups remain sensitive after retirement.

Production and instance-scoped VM recovery entries are separate. VM disposal
retains vault archives; new volumes require explicit archive removal or new
instance identity. Recovery consoles are unlogged; secret-bearing copies stay
in guest tmpfs/workstation memory. [Recovery instructions](../skillet/crates/workstation/docs/tpm.md).

References: [KeePassXC](https://keepassxc.org/docs/KeePassXC_GettingStarted),
[KDBX library](https://github.com/sseemayer/keepass-rs),
[systemd credentials](https://github.com/systemd/systemd/blob/main/docs/CREDENTIALS.md).
