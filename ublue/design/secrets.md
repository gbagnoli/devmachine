# Secret storage and delivery

UI provisioning reads a mandatory Cloudflare Zone ID and optional relative
UI prefix from the selected environment in KeePassXC, fetches the zone domain,
and derives service names from the host's declarations,
and uses shared credential delivery. Cloudflare token issuance, DNS lifecycle,
and smoke VM cleanup are implemented. Staging ACME and HTTPS acceptance passed
on 2026-10-02; production renewal remains pending.
See the [generic UI plan](../plan/GENERIC-PRIVATE-UIS.md).

Decision: use KeePassXC for durable secrets and read and write its KDBX database
directly from workstation Skillet through a Rust library. Pi-hole delivery uses
an existing entry or creates one for a fresh host. Tailscale OAuth credentials
stay on the workstation and mint short-lived, one-use enrollment keys.

## Portable storage

Planned disk recovery: production LUKS recovery keys use
`skillet/hosts/<host>/storage/root-recovery-key` and remain available off the
protected host. Root unlock uses the TPM, independently of vault access or
Skillet application-credential delivery. Disposable encryption tests generate
their own recovery material. Header backups require protected off-machine
storage; implementation and recovery checks are in the
[TPM encrypted-root plan](../plan/TPM-ENCRYPTED-ROOT.md).

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
without the old one's keyring. `skillet secret unlock` verifies the database and
establishes the same session cache without coupling unlock to provisioning.
The Cloudflare token creator is stored at `skillet/cloudflare/token-creator`.

Vault paths are group paths plus an entry title; values use the Password field.
The prefix is consistently singular, `skillet`:
KeePassXC environment groups are named `prod` and `test`; the CLI's `production`
policy maps to the `prod` group.

| Vault entry | Host credential and Podman secret name |
| --- | --- |
| `skillet/hosts/clamps/pihole/web-password` | `pihole_web_password` |
| `skillet/environments/<environment>/dns/ui-domain` | Optional relative UI prefix; defaults to `ui`, appended to the fetched zone domain |
| `skillet/environments/<environment>/dns/cloudflare-zone-id` | Environment-specific Cloudflare Zone ID lookup (`prod` or `test`) |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` | Persistent `cloudflare_acme_token`, created on first Caddy delivery |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/ddns-dns-name` | Private relative record name consumed by the shared DDNS configuration template |
| `skillet/environments/prod/hosts/<host>/cloudflare/ddns-token` | Persistent child token, created on first DDNS delivery; distinct from ACME |
| `skillet/hosts/<host>/cloudflare/acme-token` | Legacy production credential migrated on first delivery |
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

## Configuration templates

Use the generic [configuration template design](configuration-templates.md)
when service configuration combines clear policy with KeePassXC values. The
DDNS template is the first implementation.

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

The host-specific full-apply systemd unit loads its files with
`LoadCredentialEncrypted=`.
Skillet reads `$CREDENTIALS_DIRECTORY/<credential>` and sends the value through
stdin to `podman secret create`. Quadlets reference secret names with `Secret=`:
Pi-hole receives a file under `/run/secrets/` through `WEBPASSWORD_FILE`; Caddy
receives `cloudflare_acme_token` through `type=env,target=CF_API_TOKEN`.
The versioned `caddy_sites` credential contains host identity, selected UI
domain, staging policy, and the declared service/upstream list. The receiving
host checks it against its local declaration. The generated Caddyfile is
persisted under `/etc/skillet` and contains no token. A dedicated systemd apply
unit runs only after both Caddy credentials are installed. Old payloads are
rejected; redeliver after updating the environment entries. The first
production delivery migrates a legacy per-host token into the environment path.
Podman's default secret storage persists a copy on the guest disk; systemd's
credential encryption does not encrypt that copy.

Optional DDNS uses the same encrypted delivery path and a separate apply unit.
The workstation renders the shared configuration template, resolves its typed
KeePassXC references, and combines it in memory with a zone-scoped child token.
The guest mounts the Podman secret as
`/config.json`, root-only. The master token is never delivered. Production
delivery and journaled disposable ownership/cleanup are implemented; live
disposable DNS acceptance awaits its test name entry. See
[DDNS design](cloudflare-ddns.md).

## Disposable credentials

The [Cloudflare lifecycle decision](cloudflare-ui-lifecycle.md) is implemented
for credential issuance, DNS reconciliation, and disposable cleanup. No manual
smoke VM ACME token is required.

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

Opt-in `test vm provision <host> <instance> --with-ui` reads
`skillet/cloudflare/token-creator` locally and mints a 12-hour Cloudflare token
per disposable instance, restricted to the selected zone with Zone Read and DNS
Write. After Tailscale enrollment, it reconciles machine A/AAAA records for the
verified tailnet addresses, canonical UI CNAMEs, and declared alias CNAMEs.
The token supports Caddy DNS-01. DNS Write scopes access to the whole zone;
unique test names prevent record collisions, not access to other records. On
destroy, Skillet issues a short-lived cleanup token, removes only records with
the instance ownership marker, and revokes disposable tokens. Keep literal
zone names and records outside this public repository. The token creator
remains on the workstation. Staging ACME and HTTPS acceptance passed on
2026-10-02; production renewal and external access remain pending.

VM Pi-hole secrets use the same SSH, systemd, and Podman delivery path. Keep token IDs,
expiry, and owned record IDs in run metadata, without token values. Existing
VM credentials survive re-apply and reboot. Expired tokens require explicit
renewal; cleanup and recovery belong to the [VM lifecycle](smoke-vms.md).

Optional Datadog delivery resolves environment `datadog/api-key` and
`datadog/site` leaves through the shared templates. A single `datadog_config`
credential uses the existing encrypted SSH/systemd path and independently
activates the Agent phase. The Agent consumes native Podman environment secrets
for API key, site, and tags; none are written into Quadlet `Environment=` values.
Disposable delivery is explicit and uses test credentials plus an authoritative
environment tag. VM destruction does not delete historical Datadog telemetry.

References: [KeePassXC](https://keepassxc.org/docs/KeePassXC_GettingStarted),
[Rust KDBX reader](https://github.com/sseemayer/keepass-rs),
[Cloudflare token creation](https://developers.cloudflare.com/fundamentals/api/how-to/create-via-api/),
[systemd credentials](https://github.com/systemd/systemd/blob/main/docs/CREDENTIALS.md).
