# Secret storage and delivery

Decision: use KeePassXC for durable secrets and read its KDBX database directly
from workstation Skillet through a Rust library. Delivery is planned; the host
credential reader and Podman secret convergence already exist.

## Portable storage

KeePassXC is already the user's password manager, and Syncthing already moves
the encrypted database between machines. Reusing that database avoids a custom
vault format, export mechanism, or second authoritative store. Its location is
a workstation setting pointing to the existing synced `.kdbx` file.
KeePassXC owns edits and conflict resolution; Skillet reads it in memory.
Backups include the encrypted database, with recovery passwords and any key
files retained separately. Syncthing replication alone is not versioned backup.

The desktop keyring may remember the database unlock password, accessed through
Rust's Secret Service support. A new workstation can unlock the copied database
without the old workstation's keyring. The existing `cloudflare-token-creator`
keyring item needs a one-time migration into KeePassXC.

Vault paths are group paths plus an entry title; values use the Password field.
The prefix is consistently singular, `skillet`:

| Vault entry | Host credential and Podman secret name |
| --- | --- |
| `skillet/hosts/clamps/pihole/web-password` | `pihole_web_password` |
| `skillet/hosts/clamps/cloudflare/acme-token` | `cloudflare_acme_token` |
| `skillet/cloudflare/token-creator` | Workstation only |

Host entries follow `skillet/hosts/<host>/<service>/<key>`; shared credentials
follow `skillet/<service>/<key>`. Exact lookups reject missing or ambiguous
entries. Production values persist across workstation changes; absence never
implicitly generates a replacement. Rotation is explicit. Normal host re-apply
can reuse credentials already installed on that host.

## Delivery and consumption

After signed boot and base readiness, the workstation sends only the target's
selected secret values through SSH stdin using its recorded/verified host key.
The host encrypts stdin with `systemd-creds` and atomically installs root-owned
mode `0600` files at `/etc/credstore.encrypted/skillet/<credential>.cred`.
Keeping delivery after bootstrap avoids plaintext in Ignition and run artifacts.
Secret values never enter command arguments or diagnostics.

Use host-key encryption for disposable VMs; production can use TPM2 binding
after confirming hardware support and rebase behavior. Host-key encryption is
recoverable by someone with the guest disk and its key; these files are runtime
copies, while KeePassXC supplies portable recovery.

The full-apply systemd unit loads each file with `LoadCredentialEncrypted=`.
Skillet reads `$CREDENTIALS_DIRECTORY/<credential>` and sends the value through
stdin to `podman secret create`. Quadlets reference secret names with `Secret=`:
Pi-hole receives a file under `/run/secrets/` through `WEBPASSWORD_FILE`; Caddy
receives `cloudflare_acme_token` through `type=env,target=CF_API_TOKEN`.
Rotation recreates the affected container to consume the new secret.
Podman's default secret storage persists a copy on the guest disk; systemd's
credential encryption does not encrypt that copy.

## Disposable credentials

Smoke VMs get generated test passwords, never production host entries. Live
ACME testing reads `skillet/cloudflare/token-creator` locally and creates one
short-lived Cloudflare token per VM, with Zone Read and DNS Edit restricted to
the separately managed smoke-test zone. That permission covers the whole zone;
unique test names prevent accidental record collisions, not API access to
other records. Keep the literal zone outside this public repository. The
creator remains on the workstation.

VM secrets use the same SSH, systemd, and Podman delivery path. Keep token IDs,
expiry, and owned record IDs in run metadata, without token values. Existing
VM credentials survive re-apply and reboot. Expired tokens require explicit
renewal; cleanup and recovery belong to the [VM lifecycle](smoke-vms.md).

References: [KeePassXC](https://keepassxc.org/docs/KeePassXC_GettingStarted),
[Rust KDBX reader](https://github.com/sseemayer/keepass-rs),
[Cloudflare token creation](https://developers.cloudflare.com/fundamentals/api/how-to/create-via-api/),
[systemd credentials](https://github.com/systemd/systemd/blob/main/docs/CREDENTIALS.md).
