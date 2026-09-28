# Local Btrfs snapshots

Plan to run btrbk from the OS image with configuration and timer state managed
by reusable Skillet code. The first source on clamps is the independent
`/var/lib/data/syncthing` subvolume. Keep rupik's hourly schedule and retention
(`6h` minimum; `24h 31d 6m` schedule) with snapshots under
`/var/lib/data/snapshots/syncthing`. Do not snapshot `/var/lib/data` as a parent:
nested Btrfs subvolume contents, including Syncthing and Podman storage, would
not be captured. Exclude `containers`; decide Pi-hole snapshot policy separately.

Guard both convergence and the timer against a missing or wrong data mount.
Run one systemd timer instead of preserving Chef's cron job. This keeps
snapshots on the intended filesystem and avoids duplicate runs. Local
snapshots share the source disk's failure domain and are not an independent
backup. Verify recovery by restoring a disposable snapshot to a separate path
before relying on the schedule.

Status: planned. See the [implementation milestone](../plan/CLAMPS-BTRBK.md)
and [shared storage design](storage.md).
