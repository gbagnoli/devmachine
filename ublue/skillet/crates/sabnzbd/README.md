# SABnzbd

Caller-selected rootful downloader. Donbot enables it alongside putioarr, with
an identity-protected UI at `https://sabnzbd.<host>.<ui-domain>/` and a Homer
Media card. No backend port is published. Arr applications retain API-key access
over the declared media network; the browser uses the existing Tailnet user gate.

It starts paused and without a provider. No subscription or provider password is
needed for initial provisioning. Indexers belong in Prowlarr, not SABnzbd.
Provider delivery remains pending; do not put credentials into source or chat.

Skillet creates its API key in KeePassXC before delivering the related encrypted
credential set. Ordinary apply never rotates it. Restore the original vault entry
when retained native configuration or the encrypted key exists. Keys are stored
under the selected media prefix: production
`skillet/environments/prod/hosts/<host>/media/sabnzbd-api-key`, or disposable
`skillet/environments/test/hosts/<host>/instances/<instance>/media/sabnzbd-api-key`.
Both use Password; Username is unused. No manual key creation is needed.

Initial config is written only when absent, including explicit UI hostnames,
paused startup, a free-space threshold and Sonarr/Radarr/Lidarr categories.
Native edits, queue and history survive reapply. Downloads use
`/data/downloads/usenet/{incomplete,complete}`; complete categories match the Arr
client names. Libraries and Syncthing are not mounted into the downloader.
The INI contains the API key and may later contain provider credentials; it is
private native state outside Syncthing. No raw configuration backup is exported.

For an existing owned Donbot VM:

```sh
cargo run --release -p skillet -- test vm update donbot INSTANCE
cargo run --release -p skillet -- test vm provision donbot INSTANCE --with-ui --public-target VM_TEST_ADDRESS
```

The current-source runner is temporary. A matching image release is required for
persistent boot consumers to understand the new media payload/credential set.
Inspect `sabnzbd.service` and `skillet-media-reconcile.service` for runtime status.
A paused empty queue is expected until accounts and deliberate trial inputs are
configured. No protocol preferences or automatic library searches are changed.

[Usenet plan](../../../plan/DONBOT-USENET.md) ·
[Media design](../../../design/media-stack.md) ·
[Container reference](https://docs.linuxserver.io/images/docker-sabnzbd/)
