# Skillet secrets checklist

Vault: `$XDG_DATA_HOME/skillet/secrets.kdbx` (default:
`~/.local/share/skillet/secrets.kdbx`). Paths are **group path + entry title**.
Unless an entry documents other fields, put its value in **Password** and
leave **Username** empty. `<environment>` is `prod` or `test`; `<host>` is the
canonical host name. Only prepare entries for enabled modules.

Audit with `skillet secrets check` (all declared hosts, production), or
`skillet secrets check --host clamps --environment test`. It checks required fields and their formats, without provider calls or generating secrets.
Missing optional/generated/planned entries do not fail the check.

<!-- Generated from ublue/skillet/crates/workstation/src/secret-requirements.json. -->

## AirTrail

AirTrail uses its own native accounts behind the private Tailnet UI gate. Provider integrations are optional. [Setup and backup/restore](ublue/skillet/crates/airtrail/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/prod/hosts/<host>/airtrail/database-password` | Generated | Password = Skillet-generated dedicated PostgreSQL password. Skillet uses the existing value or creates it before delivery; restore the KeePassXC entry if the host already has its encrypted credential. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/airtrail/database-password` | Generated | Password = Skillet-generated disposable AirTrail database password, scoped to this test VM instance. It remains in KeePassXC after VM destruction until you remove the instance entry. |
| `skillet/environments/prod/hosts/<host>/airtrail/aerodatabox-api-key` | Optional | Password = optional AeroDataBox key from your provider account. If present, Skillet enables that integration; if absent, AirTrail uses its default lookup provider. Do not deliver this key to smoke VMs. |
| `skillet/environments/prod/hosts/<host>/airtrail/openaip-api-key` | Optional | Password = optional OpenAIP client key from your OpenAIP account settings. If present, Skillet enables the aeronautical map overlay; if absent, AirTrail continues without it. Do not deliver this key to smoke VMs. |

## Btrbk

No vault entries for the current local snapshot configuration.

## Caddy

Prepare the shared Cloudflare entries. [UI setup](ublue/skillet/crates/caddy/README.md#credentials-and-dns).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` | Generated | Skillet creates the persistent scoped token; do not create disposable VM tokens manually. |
| `skillet/hosts/<host>/cloudflare/acme-token` | Generated | Legacy production token read during migration. Do not create new entries at this path. |

## Cloudflare

Shared zone inputs for Caddy, DDNS, private Pi-hole records and the authoritative Syncthing device name.

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/cloudflare/token-creator` | Required | Create an account-owned Cloudflare API token with Account API Tokens Write and Zone Read, restricted to the selected zones. [Permissions guide](ublue/skillet/crates/workstation/docs/cloudflare.md). |
| `skillet/environments/<environment>/dns/cloudflare-zone-id` | Required | Copy the Zone ID from the selected zone's Cloudflare Overview page. |
| `skillet/environments/<environment>/dns/ui-domain` | Optional | Choose a relative UI prefix, such as ui; absent defaults to ui. Skillet appends the zone domain. Do not enter a full domain. |

## Datadog

One shared key serves production and test. No application key required for ingestion. Site is public template configuration, defaulting to datadoghq.com. [Setup](ublue/skillet/crates/datadog/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/datadog/api-key` | Required | Create an ingestion API key in Datadog Organization Settings → API Keys. [Key guide](https://docs.datadoghq.com/account_management/api-app-keys/). |
| `skillet/datadog/monitor-setup-key` | Optional | For workstation monitor administration only: create a scoped Application Key in Datadog Organization Settings → Application Keys with monitors_read and monitors_write. Never deliver this key to hosts. [Key guide](https://docs.datadoghq.com/account_management/api-app-keys/). |

## DDNS

Prepare the shared Cloudflare token creator and Zone ID. [DDNS setup](ublue/skillet/crates/ddns/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/<environment>/dns/local-prefix` | Required | Password = relative site name below the selected Cloudflare zone, shared with private Pi-hole names. Move the former host cloudflare/ddns-dns-name value here; remove the old entry after verified delivery. |
| `skillet/environments/prod/hosts/<host>/cloudflare/ddns-token` | Generated | Skillet creates the persistent scoped token; disposable tokens are lifecycle-owned. |

## Gokapi

Native accounts behind Tailscale identity authentication; no second application password. [Setup and backups](ublue/skillet/crates/gokapi/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/prod/hosts/<host>/gokapi/admin-user` | Required | Password = one lower-case Tailscale LoginName already listed in skillet/tailscale/ui-users. Choose the initial Gokapi super-admin explicitly; other allowed identities are pre-created with User rank and guest-upload permission. No Gokapi password or API key is required. |
| `skillet/environments/test/hosts/<host>/gokapi/admin-user` | Required | Password = the selected test Gokapi super-admin LoginName from the Tailscale UI allowlist. This is account configuration, shared by disposable instances; app data and ephemeral native API tokens never belong in the vault. |

## Home Assistant

Native accounts are created during HTTPS onboarding. Backup automation starts after its optional API token is supplied. [Setup](ublue/skillet/crates/homeassistant/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/prod/hosts/<host>/homeassistant/api-token` | Optional | Password = long-lived access token created in the Home Assistant administrator profile Security page after onboarding. Native backup APIs require administrator access; use a dedicated token and revoke it when replaced. Missing entry leaves backup automation unconfigured. |
| `skillet/environments/prod/hosts/<host>/homeassistant/backup-password` | Generated | Password = Skillet-generated native backup encryption password, persisted before delivery. Keep an independent vault backup; never delete or replace this recovery key while retaining encrypted archives. |

## Homer

Private static service dashboard generated from the host's declared UI services. No vault entries currently required. [Setup](ublue/skillet/crates/homer/README.md).

## Jellyfin

No vault entries. Jellyfin keeps its native accounts in application state; complete setup before publishing its public HTTPS names. [Setup](ublue/skillet/crates/jellyfin/README.md).

## Joplin

Dedicated PostgreSQL password is generated once and stored in KeePassXC before delivery. The Joplin UI is registered from the host declaration and appears in Homer. [Setup and backup/restore](ublue/skillet/crates/joplin/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/prod/hosts/<host>/joplin/database-password` | Generated | Password = Skillet-generated dedicated Joplin database password. Skillet uses the existing value or creates it before delivery. If the host already has an encrypted credential but this entry is missing, restore the KeePassXC value; Skillet will not replace it. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/joplin/database-password` | Generated | Password = Skillet-generated disposable Joplin database password, scoped to this test VM instance. It remains in KeePassXC after VM destruction until you remove the instance entry. |

## Media

Linked Arr stack: separate internal API keys, private browser UIs and environment-specific Put.io access. [Setup](ublue/skillet/crates/media/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/<environment>/hosts/<host>/media/putio-token` | Required | Password = Put.io API token from a dedicated account/application. Test requires an empty dedicated test account; no production fallback. [Media setup](ublue/skillet/crates/media/README.md). |
| `skillet/environments/prod/hosts/<host>/media/prowlarr-api-key` | Generated | Password = Skillet-generated prowlarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/prod/hosts/<host>/media/sonarr-api-key` | Generated | Password = Skillet-generated sonarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/prod/hosts/<host>/media/radarr-api-key` | Generated | Password = Skillet-generated radarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/prod/hosts/<host>/media/lidarr-api-key` | Generated | Password = Skillet-generated lidarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/prod/hosts/<host>/media/putioarr-password` | Generated | Password = Skillet-generated Transmission RPC password shared only with the three declared download clients. Ordinary apply reuses it. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/media/prowlarr-api-key` | Generated | Password = Skillet-generated prowlarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/media/sonarr-api-key` | Generated | Password = Skillet-generated sonarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/media/radarr-api-key` | Generated | Password = Skillet-generated radarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/media/lidarr-api-key` | Generated | Password = Skillet-generated lidarr API key, persisted before first bootstrap; recover the original entry if retained application state exists. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/media/putioarr-password` | Generated | Password = Skillet-generated Transmission RPC password shared only with the three declared download clients. Ordinary apply reuses it. |

## Mikochi

No application password; uses the shared Tailnet UI allowlist. [Setup](ublue/skillet/crates/mikochi/README.md).

## Network

Caller-selected native static IPv4 and personal WPA Wi-Fi. Test VMs retain DHCP; no physical network values are delivered to test. [Setup](ublue/skillet/crates/network/README.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/hosts/<host>/network/wired` | Required | Create custom fields interface, address, mask (prefix or dotted mask), gateway, dns1 and dns2. Use the wired interface name. DNS order is preserved; Username and Password are unused. |
| `skillet/hosts/<host>/network/wifi` | Required | Username = SSID; Password = WPA personal key (8–63 printable ASCII bytes or 64 hex digits). Custom fields interface, address, mask, gateway, dns1 and dns2 follow the wired schema. DNS entries may include loopback for a local resolver. |

## NUT

No vault entry required. Skillet generates a local-only monitoring password on each opted-in host; no operator login. [UPS setup](ublue/skillet/crates/nut/README.md).

## Pi-hole

Identity-protected UIs use the shared Tailscale allowlist and no application password. [Setup](ublue/skillet/crates/pihole/README.md#credentials-and-apply).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/hosts/<host>/pihole/web-password` | Generated | Only for a caller retaining native Pi-hole authentication (current hosts do not need it). Skillet generates and saves the web admin password for a fresh host; optionally create it to choose a password. For an existing host, restore its entry rather than inventing a replacement. |
| `skillet/environments/<environment>/dns/local-prefix` | Required | Password = relative site prefix below the Cloudflare zone. Shared with the default DDNS record; e.g. home (example only). |
| `skillet/environments/<environment>/dns/entries/<record>` | Optional | One entry per local device. Username/Password unused. Custom names = relative comma/newline-separated names (defaults to title); ipv4 and/or ipv6 = one address of that family. At least one address required. All selected Pi-hole hosts share this inventory. Do not duplicate generated host names. [Schema](ublue/skillet/crates/pihole/README.md#private-dns-records). |

## Plex

No vault entries. Claim the server with your Plex account through the loopback-only SSH setup tunnel before public activation. Native server identity stays in application state. [Setup](ublue/skillet/crates/plex/README.md).

## SMTP

Production-only native Postfix relay credentials; smoke machines use a local sink without these values. [SMTP setup](ublue/skillet/crates/smtp/README.md#setup).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/smtp` | Required | Create one entry using your Mailjet SMTP credentials: Username = SMTP username; Password = SMTP password. Custom fields: host = in-v3.mailjet.com; port = 587; tls = starttls (required TLS with certificate verification); sender = a Mailjet-verified sender email address. [Mailjet credentials](https://documentation.mailjet.com/hc/en-us/articles/360043229473-How-can-I-configure-my-SMTP-parameters). |

## Storage

Generated independent LUKS root recovery passphrases. [Enrollment](ublue/skillet/crates/workstation/docs/tpm.md#production-recovery-key). Password contains the printable passphrase; custom luks-uuid and recovery-state fields and the protected root-luks.header attachment are managed by Skillet.

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/hosts/<host>/storage/root-recovery-key` | Generated | Run skillet secrets root-recovery-key before finishing production provisioning. The command unlocks the vault, saves before enrollment, and verifies the key against this volume. |
| `skillet/environments/test/hosts/<host>/instances/<instance>/storage/root-recovery-key` | Generated | Run skillet test vm recovery-key HOST INSTANCE. Archived test keys remain in the vault after VM disposal; delete the instance entry before reusing its name for a new disk. |

## Syncthing

No application credential to create. Bootstrap reuses the shared Cloudflare Zone ID and token creator to resolve the device name; provider-free tests use fixture names. Existing configuration is preserved.

## Tailnet UI

Identity authorization for caller-declared private UIs. Uses the existing host-network daemon; no second login. [Setup](ublue/skillet/crates/caddy/README.md#identity-protected-uis).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/tailscale/ui-users` | Required | Password = complete Tailscale LoginName values, separated by commas or newlines; Username is unused. Obtain the identities from tailscale status --json on an authenticated client. Both environments share this explicit allowlist. |

## Tailscale

Create an OAuth client in Tailscale Trust credentials with Auth Keys Write and Devices Core Read/Write, restricted to the provisioner tag. [Tag and scope guide](ublue/skillet/crates/host-profiles/docs/tailscale.md). Skillet mints enrollment keys; do not create per-host auth-key entries.

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/tailscale/provisioner-client-id` | Required | Copy the OAuth Client ID from the creation page; use Password, not Username. |
| `skillet/tailscale/provisioner-client-secret` | Required | Copy the OAuth Client secret from the creation page. |

## UniFi

No vault entries currently required. Controller/UI accounts are managed in UniFi, separately from Skillet provisioning.
