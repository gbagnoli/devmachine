# Plex

Donbot selects the official `plexinc/pms-docker:latest` image. The image's init
process configures the Plex worker with the caller's data UID/GID; recursive
configuration ownership changes are disabled. State is persistent under
`/var/lib/data/plex/config`; disposable transcoder files use
`/var/lib/data/plex/transcode`. Repeat apply preserves native server identity,
accounts, metadata and libraries. No GPU or tuner device is granted.

Plex mounts Arr's final TV/movie paths and Lidarr's music path read-only at
`/media/tv`, `/media/movies`, `/media/music`, using the same owner and shared
SELinux labels as the writers. Downloads and Arr configuration are not mounted.
Choose those paths when creating Plex libraries.

The public caller name is `https://plex.<zone>/`, derived from the selected
Cloudflare zone, with no private UI prefix. The container advertises
`https://plex.<zone>:443` explicitly. Caddy retains native Plex authentication,
passes streaming/WebSocket requests and caches no Plex traffic. No unauthenticated
proxy/LAN bypass list is set. Do not add the Caddy subnet to Plex's allowed-without-
authentication setting. Until the native server has a claim token in its
preferences, Caddy responds **503 — Service setup is incomplete** for this site.
Complete claiming through the SSH tunnel, then reapply Caddy to activate the
public route. The token stays in Plex state and is never included in the Caddy
credential payload.

Port 32400 is published only on host IPv4/IPv6 loopback for initial setup.
Discovery ports are not published. For a disposable VM:

```sh
cargo run --release -p skillet -- test vm ssh donbot INSTANCE --forward 18400:32400
```

Open `http://127.0.0.1:18400/web` on the workstation running that command and
claim the server using your Plex account. If the workstation is remote, forward
its localhost port to your desktop as well. Production uses the equivalent
`ssh -L 18400:127.0.0.1:32400 donbot`. No claim/account token is copied into
KeePassXC or configuration source. Plex persists its own account token in its
application data. See [Plex's setup guide](https://support.plex.tv/articles/200288666-opening-plex-web-app/).

Public DNS delivery follows [Caddy ingress](../caddy/README.md#public-services).
Verify actual clients report a secure direct connection, rather than Plex Relay;
advertising a URL alone does not prove remote playback. Backup/restore and native
client acceptance remain in the [Donbot plan](../../../plan/DONBOT-MIGRATION.md).
