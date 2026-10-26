# Arr and putioarr

The media module composes Prowlarr, Sonarr, Radarr, Lidarr and putioarr, using
their upstream `latest` images. Arr browser UIs require the shared Tailnet user
allowlist through Caddy; internal calls retain separate API keys. No backend
ports are published. See the [design](../../../design/media-stack.md).

The caller provides storage paths, the data account/group and the shared media
network. Downloads, movies and TV share `/data`; Lidarr also mounts `/music`
from the caller's synchronized files. Putioarr uses separate `sonarr`, `radarr`
and `lidarr` categories and calls those container names directly.
Worker IDs and secret-file permissions resolve from that host account, so shared
Syncthing files retain the same owner even when a host uses a different UID/GID.

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
Use one active putioarr worker per Put.io account; provider-side transfer state
is shared by that account.
Generate a token with the upstream helper, opening its printed link while signed
into the intended account, then copy the printed token into the vault entry:

```sh
podman run --rm -it --entrypoint /usr/bin/putioarr ghcr.io/wouterdebie/putioarr:latest get-token
```

Generated keys live under that media prefix in production and under
`skillet/environments/test/hosts/<host>/instances/<instance>/media` for a VM.

The guest's `media reconcile` command uses the encrypted `media_credentials`
systemd credential. `skillet-media-reconcile.service` waits for authenticated
Arr readiness, tests each owned connection, then repairs links and missing
library roots. Inspect that unit and the individual application journals after
a failure. Consumer errors suppress response bodies that might contain keys.

Provision through `skillet apply donbot` for production, or on an existing
owned [smoke VM](../vm/README.md):

```sh
cargo run --release -p skillet -- test vm update donbot INSTANCE
cargo run --release -p skillet -- test vm provision donbot INSTANCE --with-ui
cargo run --release -p skillet -- test smoke donbot --instance INSTANCE --with-applications
```

Only one test instance for a host can own its shared UI DNS at a time. Complete
the current instance's lifecycle before moving that ownership to a new VM.

To validate the actual Arr image/API contracts without a Put.io account:

```sh
cargo run --release -p skillet_workstation --example media_consumer -- ../butane donbot INSTANCE
```

The runner locks and verifies the VM manifest, builds a static ignored integration
test, and uses separate temporary container names, networks and synthetic keys.
It validates bootstrap, browser/API authentication, real Prowlarr connections,
roots and restart persistence, then removes its own resources. Evidence is saved
in the run's `media-consumer.log`. Routine CI compiles this test and skips its
runtime. Full `--with-applications` acceptance additionally needs real putioarr
and its test account; a completed transfer/import remains a separate check.

References: [Prowlarr API](https://prowlarr.com/docs/api/),
[external authentication](https://wiki.servarr.com/radarr/faq#forced-authentication),
[putioarr](https://github.com/wouterdebie/putioarr).
