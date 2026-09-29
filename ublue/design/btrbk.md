# Local Btrfs snapshots

Install btrbk in the shared uCore image, mask its packaged daily timer, and
manage configuration and the hourly timer through a reusable Skillet module.
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
service reruns the shared data preparation validation before calling btrbk, so
it fails closed if the intended `/data` Btrfs mount is absent or wrong. Pi-hole
snapshot policy remains separate.

Guard both convergence and the timer against a missing or wrong data mount.
Run one systemd timer instead of preserving Chef's cron job. This keeps
snapshots on the intended filesystem and avoids duplicate runs. Local
snapshots share the source disk's failure domain and are not an independent
backup. Verify recovery by restoring a disposable snapshot to a separate path
before relying on the schedule.

Status: implementation and real-VM snapshot/restore verification are complete.
See the [implementation milestone](../plan/CLAMPS-BTRBK.md) and
[shared storage design](storage.md).
