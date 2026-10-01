# Secret storage and delivery

Planned UI generalization: select a base domain and Cloudflare zone from
environment entries in KeePassXC, derive service names from the host caller's
declarations, and share delivery across persistent and disposable targets.
The [generic UI plan](../plan/GENERIC-PRIVATE-UIS.md) defines the proposed vault
paths and migration. Existing per-service hostname entries and clamps-only
Caddy delivery below describe current code, not the intended shared interface.

Decision: use KeePassXC for durable secrets and read and write its KDBX database
directly from workstation Skillet through a Rust library. Pi-hole delivery uses
an existing entry or creates one for a fresh host. Tailscale OAuth credentials
stay on the workstation and mint short-lived, one-use enrollment keys.

## Portable storage

KeePassXC is already the user's password manager, and Syncthing already moves
the encrypted database between machines. Reusing that database avoids a custom
vault format, export mechanism, or second authoritative store. Skillet defaults
to `$XDG_DATA_HOME/skillet/secrets.kdbx` (or
`$HOME/.local/share/skillet/secrets.kdbx`); a symlink may point at the existing
synced vault. Vault-dependent commands fail if that path is absent, with an
explicit `--database` override. Disposable VM provisioning needs no vault.
Skillet resolves symlinks before writing, saves a verified encrypted replacement
atomically, and retains an encrypted backup of the prior database. It rejects a
write when the database changed since opening. KeePassXC and Syncthing can
still write concurrently, so close KeePassXC before Skillet creates an entry
and resolve any Syncthing conflict copies before retrying.
Backups include the encrypted database, with recovery passwords and any key
files retained separately. Syncthing replication alone is not versioned backup.

The workstation prompts for the database password once and caches it in a
named Linux session keyring for three hours (10,800 seconds), without
refreshing the expiry on reads. Skillet creates and joins this keyring itself,
and links it from the per-user keyring so it survives separate CLI invocations.
The password key remains in the session keyring, where the process has the
permissions needed to set its expiry. If the kernel denies expiry, Skillet
removes the new key and continues without a cache. `skillet secret lock`
removes a cached key early. A new workstation can unlock the copied database
without the old one's keyring.
The existing `cloudflare-token-creator` keyring item still needs migration.

Vault paths are group paths plus an entry title; values use the Password field.
The prefix is consistently singular, `skillet`:

| Vault entry | Host credential and Podman secret name |
| --- | --- |
| `skillet/hosts/clamps/pihole/web-password` | `pihole_web_password` |
| `skillet/hosts/clamps/caddy/pihole-hostname` | Included in encrypted `caddy_sites` config |
| `skillet/hosts/clamps/caddy/syncthing-hostname` | Included in encrypted `caddy_sites` config |
| `skillet/hosts/clamps/cloudflare/acme-token` | `cloudflare_acme_token` |
| `skillet/cloudflare/token-creator` | Workstation only |
| `skillet/tailscale/provisioner-client-id` | Workstation only; OAuth Client ID |
| `skillet/tailscale/provisioner-client-secret` | Workstation only; OAuth Client secret |

Host entries follow `skillet/hosts/<host>/<service>/<key>`; shared credentials
follow `skillet/<service>/<key>`. The Tailscale OAuth client is a workstation
master credential used to mint one-use host enrollment keys; it is never
delivered to a host. Exact lookups reject ambiguous entries and
entries with empty Password fields. A missing Pi-hole entry is generated only
after Skillet successfully opens the existing vault and confirms over SSH that
the host has neither an encrypted Pi-hole credential nor a Podman secret. If
the host has either one or its state cannot be checked, Skillet refuses to
create a replacement. Production values persist across workstation
changes; rotation is an explicit vault edit and redelivery. Normal host
re-apply can reuse credentials already installed on that host.

## Delivery and consumption

After signed boot and base readiness, the workstation sends only the target's
selected secret values through SSH stdin using its recorded/verified host key.
The host Rust command encrypts stdin with `systemd-creds` and atomically
installs root-owned mode `0600` files at
`/etc/credstore.encrypted/skillet/<credential>.cred`.
Keeping delivery after bootstrap avoids plaintext in Ignition and run artifacts.
Secret values never enter command arguments or diagnostics.

Use host-key encryption for disposable VMs; current production delivery also
uses host-key encryption. Production can use TPM2 binding
after confirming hardware support and rebase behavior. Host-key encryption is
recoverable by someone with the guest disk and its key; these files are runtime
copies, while KeePassXC supplies portable recovery.

The full-apply systemd unit loads each file with `LoadCredentialEncrypted=`.
Skillet reads `$CREDENTIALS_DIRECTORY/<credential>` and sends the value through
stdin to `podman secret create`. Quadlets reference secret names with `Secret=`:
Pi-hole receives a file under `/run/secrets/` through `WEBPASSWORD_FILE`; Caddy
receives `cloudflare_acme_token` through `type=env,target=CF_API_TOKEN`.
The `caddy_sites` credential contains the two private UI hostnames and the
ACME staging flag. The generated Caddyfile is persisted under `/etc/skillet`
and contains no token. A dedicated systemd apply unit runs only after both
Caddy credentials are installed. Rotation recreates the affected container to
consume the new secret.
Podman's default secret storage persists a copy on the guest disk; systemd's
credential encryption does not encrypt that copy.

## Disposable credentials

Smoke VMs get generated test passwords, never production host entries. Tailscale
enrollment reads `skillet/tailscale/provisioner-client-id` and
`skillet/tailscale/provisioner-client-secret` locally, then mints a one-use,
preauthorized, non-ephemeral key tagged for that smoke VM. The key expires in
one hour if unused and is delivered through the same SSH, systemd, and Podman
secret path as other credentials. The workstation verifies the VM's tailnet
addresses and saves its device ID and addresses in mode-0600 run metadata. On
destroy, Skillet uses `devices:core` to find and remove only the device with the
recorded name, smoke tag, and identity. If enrollment or deletion is
interrupted, pending metadata remains so cleanup can be retried. Production
uses the server tag and the same one-use-key path.

Live UI/ACME testing will read `skillet/cloudflare/token-creator` locally and mint
one short-lived Cloudflare token per VM, with Zone Read and DNS Edit restricted
to the existing smoke-test zone. Once a VM joins the tailnet, the workstation
will create DNS-only A/AAAA records pointing at its tailnet address; the VM
token also supports Caddy's DNS-01 challenge. DNS Edit covers the whole zone;
unique test names prevent collisions, not access to other records. Production
UI records belong in the existing production zone and use a separate scoped
credential. Keep literal zone names and records outside this public repository.
The token creator remains on the workstation. Cloudflare token issuance and
record management are planned; current smoke provisioning only enrolls
Tailscale and provisions Pi-hole. Production Caddy credentials can be delivered
with `secret deliver clamps caddy`; smoke-specific staging and delivery are
not yet wired into VM provisioning.

VM Pi-hole secrets use the same SSH, systemd, and Podman delivery path. Keep token IDs,
expiry, and owned record IDs in run metadata, without token values. Existing
VM credentials survive re-apply and reboot. Expired tokens require explicit
renewal; cleanup and recovery belong to the [VM lifecycle](smoke-vms.md).

References: [KeePassXC](https://keepassxc.org/docs/KeePassXC_GettingStarted),
[Rust KDBX reader](https://github.com/sseemayer/keepass-rs),
[Cloudflare token creation](https://developers.cloudflare.com/fundamentals/api/how-to/create-via-api/),
[systemd credentials](https://github.com/systemd/systemd/blob/main/docs/CREDENTIALS.md).
