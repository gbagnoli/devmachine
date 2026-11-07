# File Browser Quantum

Shared Syncthing hosts serve Quantum at `https://files.<host>.<ui-domain>`.
The official `ghcr.io/gtsteffaniak/filebrowser:2.0.2-stable` image includes previews.
The trial is pinned; automatic application upgrades are disabled.

## Access and storage

The existing Caddy tailnet identity gate supplies `X-Skillet-User`. Quantum uses
native proxy login with password login disabled. Allowed people receive separate
ordinary application accounts over the same shared files, with view, download,
upload, edit and delete permissions. Public sharing and API-token creation are
disabled. There is no additional application password or exposed host port.
Private backend isolation remains required: header authentication trusts its proxy.

The file scope remains `/var/lib/data/syncthing/data`, using the same host owner
and user-namespace mapping as Syncthing. Other applications, Syncthing config and
the personal Syncthing instance are outside this scope. SQLite state and preview
cache live separately in `/var/lib/data/filebrowser`. The application owns that
state; repeat apply preserves users, sessions and preferences. Source YAML is
managed at `/etc/skillet/filebrowser/config.yaml`. Existing file backups still
cover synchronized documents; database recovery/export is a separate follow-up.

Quantum can show PDFs using a browser's native viewer. Mobile Chromium currently
uses a download/open fallback rather than an embedded multipage viewer. Verify
this on the actual phone before accepting the replacement.

## Disposable trial

From `ublue/skillet`:

```sh
cargo run --release -p skillet -- test vm create clamps quantum --port 2203
```

Before full provisioning, expand the plain-root VM for the fleet's application
images using the [owned disk growth commands](../vm/README.md#owned-guest-assertions).
The source image has only 10 GiB; use 64 GiB for this trial.

```sh
cargo run --release -p skillet -- secrets unlock
cargo run --release -p skillet -- test vm provision clamps quantum --with-ui
cargo run --release -p skillet -- test smoke clamps --instance quantum --with-applications
cargo run --release -p skillet_workstation --example filebrowser_trial -- ../butane clamps quantum
```

If the VM was created before the current build, run `test vm update clamps quantum`
before provisioning and again after smoke: readiness restores its original captured
bootstrap binaries. This preserves capture evidence while explicitly deploying the
current build.

The final command seeds a `Quantum trial` folder with harmless text and a two-page
PDF, using the recorded VM ownership and declared file identity.
Open the test environment's `files` hostname from an allowed personal tailnet
device. Inspect `filebrowser.service`, `skillet-tailnet-ui.service` and
`caddy.service`. Test a multipage PDF, folder navigation, upload, rename, download
and delete; a successful HTTP probe does not establish phone usability.

## Existing installations

After merging and deliberate deployment, full apply verifies the old Mikochi
Quadlet against its completed Skillet receipt, stops it and retains it as
`mikochi.container.skillet-legacy`. Custom edits or missing/mismatched receipts
require operator inspection. Reload completion is recorded for interrupted retries.
No user files or application databases are deleted. Redeliver Caddy configuration
through remote apply to select the new upstream and remove the legacy frontend
refresh workaround. Retained older Caddy payloads still parse.

For a deliberate rollback, stop Quantum, deploy the previous Skillet binaries
and run that version's full apply with its Caddy delivery. It regenerates the old
Mikochi definition; synchronized files retain the same paths and ownership.
Retain Quantum's state directory if planning to resume the trial.

Design: [file browsing](../../../design/file-browsing.md).
Trial/remaining acceptance: [plan](../../../plan/FILEBROWSER-QUANTUM.md).
