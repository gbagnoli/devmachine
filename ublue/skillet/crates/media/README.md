# Arr and putioarr

The media module composes Prowlarr, Sonarr, Radarr, Lidarr and putioarr, using
their upstream `latest` images. Arr browser UIs require the shared Tailnet user
allowlist through Caddy; internal calls retain separate API keys. No backend
ports are published. See the [design](../../../design/media-stack.md).

The caller provides storage paths, numeric data identity and the shared media
network. Downloads, movies and TV share `/data`; Lidarr also mounts `/music`
from the caller's synchronized files. Putioarr uses separate `sonarr`, `radarr`
and `lidarr` categories and calls those container names directly.

Provisioning generates each Arr API key and the putioarr RPC password once.
Keep the vault entries: missing keys alongside retained credentials require
recovery. Reapply preserves existing app configuration and reconciles only
reserved `Skillet …` API links. Do not reuse those names for manual links.
Prowlarr performs full indexer synchronization; add indexer accounts in Prowlarr.
No cleanup job removes completed files automatically.

Supply a Put.io API token in Password at
`skillet/environments/<environment>/hosts/<host>/media/putio-token`.
For smoke tests use an empty dedicated test account: putioarr can process account
transfers as soon as it starts. No production provider token is copied to test.
Generated keys live under that media prefix in production and under
`skillet/environments/test/hosts/<host>/instances/<instance>/media` for a VM.

The guest's `media reconcile` command uses the encrypted `media_credentials`
systemd credential. `skillet-media-reconcile.service` waits for authenticated
Arr readiness, tests each owned connection, then repairs links and missing
library roots. Inspect that unit and the individual application journals after
a failure. Consumer errors suppress response bodies that might contain keys.

References: [Prowlarr API](https://prowlarr.com/docs/api/),
[external authentication](https://wiki.servarr.com/radarr/faq#forced-authentication),
[putioarr](https://github.com/wouterdebie/putioarr).
