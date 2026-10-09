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

## Btrbk

No vault entries for the current local snapshot configuration.

## Caddy

Prepare the shared Cloudflare entries. [UI setup](ublue/skillet/README.md#caddy-ui-credentials).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token` | Generated | Skillet creates the persistent scoped token; do not create disposable VM tokens manually. |
| `skillet/hosts/<host>/cloudflare/acme-token` | Generated | Legacy production token read during migration. Do not create new entries at this path. |

## Cloudflare

Shared inputs for Caddy and DDNS.

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/cloudflare/token-creator` | Required | Create an account-owned Cloudflare API token with Account API Tokens Write and Zone Read, restricted to the selected zones. [Permissions guide](ublue/skillet/README.md#secret-setup-and-delivery). |
| `skillet/environments/<environment>/dns/cloudflare-zone-id` | Required | Copy the Zone ID from the selected zone's Cloudflare Overview page. |
| `skillet/environments/<environment>/dns/ui-domain` | Optional | Choose a relative UI prefix, such as ui; absent defaults to ui. Skillet appends the zone domain. Do not enter a full domain. |

## Datadog

One shared key serves production and test. No application key required for ingestion. Site is public template configuration, defaulting to datadoghq.com. [Setup](ublue/skillet/README.md#datadog).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/datadog/api-key` | Required | Create an ingestion API key in Datadog Organization Settings → API Keys. [Key guide](https://docs.datadoghq.com/account_management/api-app-keys/). |
| `skillet/datadog/monitor-setup-key` | Optional | For workstation monitor administration only: create a scoped Application Key in Datadog Organization Settings → Application Keys with monitors_read and monitors_write. Never deliver this key to hosts. [Key guide](https://docs.datadoghq.com/account_management/api-app-keys/). |

## DDNS

Prepare the shared Cloudflare token creator and Zone ID. [DDNS setup](ublue/skillet/README.md#optional-cloudflare-ddns).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/environments/<environment>/hosts/<host>/cloudflare/ddns-dns-name` | Required | Choose the relative DNS record name, not a full domain. |
| `skillet/environments/prod/hosts/<host>/cloudflare/ddns-token` | Generated | Skillet creates the persistent scoped token; disposable tokens are lifecycle-owned. |

## Pi-hole

[Delivery](ublue/skillet/README.md#secret-setup-and-delivery).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/hosts/<host>/pihole/web-password` | Generated | Skillet generates and saves the web admin password for a fresh host; optionally create it to choose a password. For an existing host, restore its entry rather than inventing a replacement. |

## SMTP

Production-only native Postfix relay credentials; smoke machines use a local sink without these values. [SMTP plan](ublue/plan/CLAMPS-EMAIL.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/smtp` | Required | Create one entry using your Mailjet SMTP credentials: Username = SMTP username; Password = SMTP password. Custom fields: host = in-v3.mailjet.com; port = 587; tls = starttls (required TLS with certificate verification); sender = a Mailjet-verified sender email address. [Mailjet credentials](https://documentation.mailjet.com/hc/en-us/articles/360043229473-How-can-I-configure-my-SMTP-parameters). |

## Storage (planned)

No entry required today. Keep recovery material off the encrypted host. [Encryption plan](ublue/plan/TPM-ENCRYPTED-ROOT.md).

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/hosts/<host>/storage/root-recovery-key` | Planned | LUKS recovery key; prepare through the future TPM/LUKS workflow. |

## Syncthing

No vault entries currently required; Skillet manages the private UI configuration.

## Tailscale

Create an OAuth client in Tailscale Trust credentials with Auth Keys Write and Devices Core Read/Write, restricted to the provisioner tag. [Tag and scope guide](ublue/skillet/README.md#tailscale-setup-for-skillet). Skillet mints enrollment keys; do not create per-host auth-key entries.

| Entry | Status | Fields / how to obtain them |
| --- | --- | --- |
| `skillet/tailscale/provisioner-client-id` | Required | Copy the OAuth Client ID from the creation page; use Password, not Username. |
| `skillet/tailscale/provisioner-client-secret` | Required | Copy the OAuth Client secret from the creation page. |

## UniFi

No vault entries currently required. Controller/UI accounts are managed in UniFi, separately from Skillet provisioning.
