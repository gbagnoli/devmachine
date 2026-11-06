# Local Btrfs snapshots

Install btrbk in the shared uCore image, mask its packaged daily timer, and
render vendor configuration and the hourly timer through a reusable Skillet module.
Bundles own `/usr/share/skillet/btrbk` and the two `/usr/lib/systemd/system`
units. Runtime retains storage checks and activation.
Each host opts in by passing explicit
subvolume paths relative to `/var/lib/data`; an empty list opts out. Clamps
selects only `syncthing`, stored at `/var/lib/data/syncthing`, with snapshots
under `/var/lib/data/snapshots/syncthing`. The module rejects absolute paths,
traversal, and the data root, and requires each selected source to be an existing
Btrfs subvolume. It never defaults to snapshotting the parent or `containers`.

Selected snapshot sources must be leaf subvolumes. Nested subvolumes inside a
selected source are unsupported: Btrfs snapshots do not include their contents.
Syncthing's data layout is designed without nested subvolumes, and this is an
invariant of the current policy rather than a recursively discovered source
list. A future need for nested subvolumes requires an explicit design change.

Keep rupik's hourly schedule and retention (`6h` minimum; `24h 31d 6m`). The
service runs its Rust caller's `btrbk prepare` validation before calling btrbk, so
it fails closed if the intended `/data` Btrfs mount is absent or wrong. Pi-hole
snapshot policy remains separate.

Guard both convergence and the timer against a missing or wrong data mount.
Run one systemd timer instead of preserving Chef's cron job. This keeps
snapshots on the intended filesystem and avoids duplicate runs. Local
snapshots share the source disk's failure domain and are not an independent
backup. Verify recovery by restoring a disposable snapshot to a separate path
before relying on the schedule.

The image ownership marker selects vendor mode; incomplete/mismatched vendor
files fail closed. Retained `/etc` definitions are retired only when their bytes
and successful legacy receipt match. Same-filesystem, no-replace relocation
preserves recovery copies and metadata, including across interrupted retries.
Unknown edits and symlinks are refused. Enablement is explicitly repointed;
success is recorded only after timer activation. The retained migration-capable
caller restores legacy definitions on an older image, giving rollback an explicit
configuration reconciliation step. Existing bootstrap binaries and the shared
Podman storage helper are otherwise unchanged by this pilot.

Instructions: [btrbk](../skillet/crates/btrbk/README.md).
Storage contract: [design](storage.md).
