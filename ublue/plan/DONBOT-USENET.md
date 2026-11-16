# Donbot Usenet trial

Status: planning, 2026-11-16. No downloader, credentials or subscriptions have
been added. Jellyfin onboarding can wait independently. Follow
[Donbot](DONBOT-MIGRATION.md) and the [media design](../design/media-stack.md).

## Proposed first setup

Use **SABnzbd** as the first client, with `lscr.io/linuxserver/sabnzbd:latest`.
It matches the existing LSIO UID/GID convention and offers configuration and
category APIs. NZBGet is a viable alternative, but implementing both immediately
adds no useful acceptance coverage. Confirm SABnzbd before implementation.

Start with one provider and one NZB indexer. A provider supplies NNTP article
access; an indexer supplies searchable NZB results. A downloader alone does not
supply either account. Select subscriptions separately; this plan buys nothing.
Assess provider retention, completion, quota/speed, allowed connections, server
location and TLS; assess indexer registration, API/RSS quotas and TV/movie/music
coverage. Start with a short trial/month rather than multiple overlapping plans.
Extra block/fallback providers can wait until observed failures justify them.

Flow:

```text
Indexer --NZB search/RSS--> Prowlarr --> Sonarr / Radarr / Lidarr
                                           |
                                     NZB download request
                                           v
Provider --NNTP over TLS--> SABnzbd --> completed files --> Arr import
                                                           |
                                                   existing libraries
                                                           |
                                                     Jellyfin / Plex
```

Prowlarr synchronizes indexers into Arr. Arr sends NZB jobs directly to SABnzbd;
Prowlarr is not the download engine. Keep putioarr working alongside Usenet.
Choose protocol preference separately from client priority: adding SABnzbd does
not make every failed Put.io/torrent job automatically fall back to Usenet.

## 1. Make download clients selectable

Current assumptions to remove:

- `media::App::ALL` includes putioarr and drives host declarations, container
  startup and reconciliation dependencies.
- `media::Credentials` always requires Put.io credentials; workstation delivery
  always resolves its token and creates its RPC password.
- `reconcile::download_client` always builds the Transmission implementation and
  reserves `Skillet putioarr`.
- Current media tests/smoke requirements assume that whole fixed stack.

Keep Arr applications separate from selected download backends. Add a small
explicit caller declaration for enabled clients; avoid a plugin framework or a
trait per downloader. Existing declarations retain putioarr-only behavior.
Support putioarr-only, SABnzbd-only and both. Credential presence must not select
a backend. Do not make provider-free CI or SABnzbd-only tests require Put.io.
Version changed credential payloads explicitly, retaining the old reader contract
where needed. Update audit requirements, monitoring, readiness, image rendering,
consumer dependencies and temporary-runner coverage from the same declaration.

**Exit:** selection-matrix tests prove no disabled container, provider lookup,
credential requirement, dependency or reconciliation call; existing Put.io links
and unrelated app settings survive unchanged. Existing acceptance remains valid.

## 2. Add the reusable SABnzbd service

Create a focused `skillet_sabnzbd` crate for native bootstrap/configuration,
container and monitoring declarations. `skillet_media` owns Arr linking. The host
caller supplies paths, data identity and networks; Donbot opts in explicitly.

- Rootful Quadlet, bridge networking, auto-update and storage dependencies.
- Worker UID/GID comes from the same host data account as Arr, not image defaults.
- Private Caddy UI `sabnzbd.<host>.<ui-domain>`, Tailnet user authorization,
  Homer Media registration, no published backend port and no public DNS alias.
- Keep API-key authentication for machine calls. Avoid a second browser login
  only after native wizard, host checks, CSRF and settings requests pass through
  the real authorized proxy. Do not globally disable hostname/CSRF checks.
- Bootstrap a fresh native INI with its generated API key before first start.
  Preserve unrelated native edits and queue/history. Use supported APIs for
  declared server/category updates; do not overwrite a live INI on every apply.
- Capture the actual image/config/API contract in a native consumer test before
  relying on schema fields or LSIO secret-file conventions.

Storage (caller-selected parent; Donbot examples):

| Host | Container / use |
| --- | --- |
| `/var/lib/data/media/config/sabnzbd` | `/config`, persistent native state |
| `/var/lib/data/media/files/downloads/usenet` | `/data/downloads/usenet`, writable downloads only |
| `.../usenet/incomplete` | in-progress download/repair/unpack |
| `.../usenet/complete/{sonarr,radarr,lidarr}` | separate completed categories |

Do not mount libraries or Syncthing into SABnzbd. Arr already sees the same
absolute download paths under `/data`; no remote path mapping should be needed.
TV/movie import can rename/hardlink only after testing the real filesystem.
Music goes to `/music` in the separate Syncthing subvolume and may require a
copy even though both paths use Btrfs. Downloads stay outside snapshot/Syncthing
scope. Bound disk consumption and pause before exhausting repair/unpack space.
Do not introduce age-based deletion or change existing Put.io paths.

**Exit:** official image starts from absent state, accepts the generated key,
preserves it across repeat apply/restart, and exposes the real browser UI only
through authorized Caddy. Foreign peers, tagged servers and direct backends are
denied. API requests without the key fail even on the media bridge.

## 3. Credentials and provider configuration

Planned credential layout, not yet requirements of `secrets check`:

- Generated SABnzbd full API key: existing media prefix plus `sabnzbd-api-key`.
  Production prefix is `skillet/environments/prod/hosts/<host>/media`; disposable
  prefix is `skillet/environments/test/hosts/<host>/instances/<instance>/media`.
  Use-or-create, persist before delivery, never rotate silently.
- Provider account: one structured entry at
  `skillet/usenet/providers/<profile>/account`, Username and Password fields.
  Share one account entry where appropriate rather than duplicating it per host.
  A test account/profile is separately selected and must never be inferred from
  production configuration. Confirm provider connection limits before concurrent
  real workers use any shared account.
- Clear server endpoint, port, connection count, category and retention policy
  belong in reviewed typed configuration/templates. Start with verified NNTP TLS
  (usually 563) and conservative connections within the account's allowance.
  No plaintext fallback and no disabled certificate verification.

A typed provider reference selects the structured account entry. Read Username
and Password through the existing structured vault API; the current `$secret`
template marker reads Password only, so do not assume it can select Username.
Any generalized field-reference extension needs its own schema/audit tests.
Resolve only selected accounts at the workstation; deliver encrypted credentials
and Podman secrets through the existing path. No literal secrets in argv, unit
environment, source, logs or recordings. SABnzbd may persist credentials in its
writable native INI: converge restrictive file permissions and document that
native disk copy; mounting a secret does not eliminate it.

First indexer setup stays manual in Prowlarr, preserving its ownership of indexer
settings. The indexer supplies a separate API key; an NNTP provider login is not
that key. Skillet does not need to read indexer credentials for this slice.
Automated indexer provisioning, if requested later, gets a separate typed schema
and audited requirements rather than storing configuration JSON in KeePassXC.

**Exit:** `secrets check` validates selected account fields, lists optional inputs
correctly, isolates test/production and reports missing values without exposing
them. A provider-free VM mode can use synthetic keys without any external login.
Update the requirements catalog and generated root checklist when implementing,
not by making planned entries mandatory now.

## 4. Wire Arr and indexers

Reconcile `Skillet SABnzbd` independently in Sonarr, Radarr and Lidarr, using
native `Sabnzbd` schemas and consumer Test endpoints before writes. Set container
DNS/port/API key and the matching category. Preserve IDs, unrelated clients,
quality profiles, library roots and existing `Skillet putioarr` links.

Use Prowlarr's UI to add/test one NZB indexer and check sync to all three apps;
verify relevant category mappings, API/RSS limits and actual music coverage.
Do not trigger a whole-library search or enable automatic grabs for the trial.
Initially retain current protocol preference. After a deliberate sample succeeds,
choose whether to prefer Usenet via each app's supported protocol/delay policy;
client priority alone is not a cross-protocol preference. Configure only declared
policy, not a blanket rewrite of all delay/quality profiles.

**Exit:** all three real Arr connection tests pass, each sees the NZB indexer,
and repeated reconciliation creates no duplicate clients or links. Masked-key
handling follows existing supported consumer tests. Selection changes preserve
native configuration/downloads; retiring an owned client is explicit and must
not delete queues or user-managed clients.

## 5. Validate and adopt

1. **CI, no providers/vault:** selection and credential migrations; native INI
   bootstrap; redaction and permission tests; SABnzbd/Arr API fixtures covering
   categories, retained IDs and rejected keys; generated runtime commands parse
   in the actual generic image binary. Real image checks prove the contract.
2. **Owned Donbot VM, no provider:** fresh SABnzbd startup, real Arr Test calls,
   Caddy browser/API authorization, restart and repeat convergence. Prove a small
   manually staged completed file imports correctly; label this as import-only,
   not proof of NNTP downloading, repair or extraction.
3. **Opt-in live trial:** require explicitly selected test credentials and a
   provider/indexer-authorized sample. Check TLS, search, one download, normal
   repair/unpack and each app's import path. Use a controlled repair fixture for
   corruption recovery; do not infer repair coverage from a healthy download.
   Check bad credentials, unavailable provider and low disk space safely.
4. **Persistence:** compare keys, native state and link IDs after reapply and
   normal reboot on a matching released image; verify private ingress isolation
   again. Keep evidence in the owned run and central acceptance record.
5. **Production adoption:** explicitly select the production account and desired
   protocol policy on Donbot. Retain putioarr until replacement is requested;
   no Calculon configuration changes or production library searches in VM tests.

The first trial keeps persistent SABnzbd state on disk but does not export raw
INI/queue state to Syncthing: native settings can contain provider passwords and
API keys. Disaster recovery initially reconstructs managed config from source
and vault; interrupted jobs may need resubmission and history is not restored.
A queue/history backup requirement needs a separate encrypted-backup design;
do not put archives into KDBX or silently synchronize plaintext credentials.
Declare process/readiness monitoring with the service; leave a new metrics
collector/dashboard outside this initial slice.

## Sources checked 2026-11-16

- [SABnzbd image and worker identity](https://docs.linuxserver.io/images/docker-sabnzbd/)
- [SABnzbd configuration API](https://sabnzbd.org/wiki/configuration/4.5/api)
- [NNTP TLS, connections and certificate verification](https://sabnzbd.org/wiki/configuration/4.5/servers)
- [SABnzbd categories](https://sabnzbd.org/wiki/configuration/4.5/categories)
- [Hostname verification settings](https://sabnzbd.org/wiki/configuration/4.5/special)
- [Servarr download/protocol settings](https://wiki.servarr.com/sonarr/settings)
- [Prowlarr settings](https://wiki.servarr.com/prowlarr/settings)
- [NZBGet alternative](https://nzbget.com/documentation/how-to-link-nzbget-with-sonarr-radarr-and-other-tools/)

These are contract references, not proof that a moving `latest` image supports
an untested bootstrap field. Record actual consumer versions at implementation.
