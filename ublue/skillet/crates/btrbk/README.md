# Btrbk snapshots

The image supplies btrbk and masks its packaged daily timer. Callers select
`ServiceConfig::Btrbk` with source subvolume paths **relative to `/var/lib/data`**.
The module accepts only existing leaf Btrfs subvolumes; an empty selection opts
out. Never select the entire data root or nested subvolumes.

Clamps/joey select `syncthing`. Snapshots are under
`/var/lib/data/snapshots/syncthing`. Configuration is `/etc/btrbk/btrbk.conf`,
with hourly `skillet-btrbk.timer` and retention `6h` minimum / `24h 31d 6m`.
Apply the selected host after its application storage exists. Missing/wrong
`/data` mounts cause both convergence and timer execution to fail.

Inspect without creating a snapshot:

```sh
sudo btrbk -n -v run
systemctl status skillet-btrbk.timer
journalctl -u skillet-btrbk.service
```

For a real snapshot, run `sudo btrbk run`. Restore into a separate temporary
path before comparing bytes; never overwrite live Syncthing data for an exercise.
These snapshots share the source disk and do not replace an off-machine backup.

Designs: [snapshot policy](../../../design/btrbk.md),
[storage](../../../design/storage.md).
