# Btrbk snapshots

The image supplies btrbk and masks its packaged daily timer. Callers select
`ServiceConfig::Btrbk` with source subvolume paths **relative to `/var/lib/data`**.
The module accepts only existing leaf Btrfs subvolumes; an empty selection opts
out. Never select the entire data root or nested subvolumes.

Clamps/joey select `syncthing`. Snapshots are under
`/var/lib/data/snapshots/syncthing`, with hourly `skillet-btrbk.timer` and
retention `6h` minimum / `24h 31d 6m`.
Apply the selected host after its application storage exists. Missing/wrong
`/data` mounts cause both convergence and timer execution to fail.

Inspect without creating a snapshot:

```sh
sudo btrbk --config /usr/share/skillet/btrbk/btrbk.conf -n -v run
systemctl status skillet-btrbk.timer
journalctl -u skillet-btrbk.service
```

For a real snapshot, run `sudo systemctl start skillet-btrbk.service`. Restore into a separate temporary
path before comparing bytes; never overwrite live Syncthing data for an exercise.
These snapshots share the source disk and do not replace an off-machine backup.

Image bundles include caller-selected configuration under `/usr/share/skillet/btrbk`
and units under `/usr/lib/systemd/system`. Their guard is
`/usr/bin/skillet-<host> btrbk prepare`, which checks the mount and every selected
source before preparing snapshot directories. No workstation credentials are needed.

After booting the new image, ordinary host apply migrates verified legacy files.
Use matching generic and caller binaries; workstation apply deploys both by default.
For this module alone, run `sudo /var/usrlocal/bin/skillet-<host> btrbk apply`
using a migration-capable caller; after migration `/usr/bin/skillet-<host>` works
too. Matching legacy files move to sibling `.skillet-legacy` recovery copies,
and the timer is reenabled against its vendor definition. Interrupted attempts can
be retried. A subsequent retirement with an existing recovery copy retains an
additional `.skillet-retired-*/retired` copy beside it. Recovery copies are not deleted automatically.

The successful activation receipt is `/var/lib/skillet/btrbk/timer.applied`.
Custom files, symlinks, mismatched receipts or incomplete vendor output cause an
error before retirement. Preserve and resolve the reported path, then retry;
do not remove the receipt to bypass the ownership check. A running snapshot must
finish before migration. Keep the migration-capable `/var/usrlocal` caller for
OS rollback: on an older image without vendor definitions, its module apply
converges legacy `/etc` files and enablement. OSTree rollback uses the previous
deployment's `/etc`; run module/host apply to reconcile the shared `/var` activation
receipt. Missing legacy definitions are recreated; later edits are preserved.
Legacy inspection uses `--config /etc/btrbk/btrbk.conf`.

Designs: [snapshot policy](../../../design/btrbk.md),
[storage](../../../design/storage.md).
