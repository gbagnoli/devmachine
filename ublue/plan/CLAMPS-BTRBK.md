# Next milestone: btrbk snapshots on clamps

Status: ready to implement after the Syncthing slice. Scope is local snapshot
parity for rupik; remote backup and Pi-hole snapshot policy are separate work.
The [snapshot design](../design/btrbk.md) records the chosen scope and why.

## Starting point

- Rupik's [Chef recipe](../../site-cookbooks/rupik/recipes/btrbk.rb) runs btrbk
  hourly against `/srv/sync`, with `timestamp_format long`,
  `snapshot_preserve_min 6h` and
  `snapshot_preserve 24h 31d 6m`.
- Clamps now stores Syncthing in the real Btrfs subvolume
  `/var/lib/data/syncthing`; `/var/lib/data` is mounted from `/data`. The
  `containers` subvolume must not be captured by a parent-directory snapshot.
- Fedora [packages btrbk](https://packages.fedoraproject.org/pkgs/btrbk/btrbk/)
  with systemd units. Verify the package and timer on the actual uCore base.
  Upstream documents the [volume/subvolume layout and dry run](https://github.com/digint/btrbk#example-local-regular-snapshots).

## Implementation sequence

1. In `ucore-images`, add `btrbk` to the shared OS recipe
   `recipes/ucore-common.yml` if absent from the base. Build the common image
   before the host images as their CI requires. Do not install packages at
   runtime or copy Chef's source-build recipe.
2. Add a reusable Skillet btrbk module. Guard the verified `/var/lib/data`
   mount before creating anything. Ensure `/var/lib/data/snapshots/syncthing`
   exists, and manage `/etc/btrbk/btrbk.conf` idempotently. Configure
   `volume /var/lib/data`, `snapshot_dir snapshots/syncthing`, and
   `subvolume syncthing`, with rupik's timestamp and retention values. Do not snapshot
   `containers` or the parent `data` subvolume. Keep Pi-hole out of this first
   slice because rupik did not snapshot it.
3. Enable exactly one hourly systemd timer, using the packaged unit if its
   schedule matches. Otherwise manage a unit/drop-in with an hourly schedule.
   The service must verify the actual `/data` Btrfs mount on every run, using
   the same checks as `butane/includes/data-storage.bu`; a mountpoint assertion
   alone does not reject a wrong filesystem. Do not enable a second packaged
   timer or create a cron job. Apply the shared module from clamps' host
   convergence only after Syncthing storage exists.
4. Add isolated module tests for rendered config, repeat apply, and missing or
   wrong data mount. Keep tests in a separate `tests.rs`; use Rust resources
   rather than embedded shell.
5. Rebuild/rebase a named disposable clamps VM onto the image with btrbk,
   provision dummy credentials, and validate the real systemd timer and Btrfs
   subvolume. Use `ublue/butane/bin/test-vm` and its README; the previous VM
   manifests on this workstation currently report `missing`.

## Exit criteria

- Workspace format, pedantic Clippy, unit tests, and the CI container command
  pass. The named VM smoke runs with real systemd and Podman.
- `btrbk -n -v run` targets only the intended Syncthing subvolume; a real run
  creates a read-only snapshot under the intended directory. Repeated Skillet
  apply preserves the config, timer, and existing snapshots. The timer remains
  active after reboot and runs at the intended hourly cadence.
- Put a disposable sentinel in the VM's Syncthing data, snapshot it, change
  the live file, and restore the snapshot into a **separate temporary path**.
  Verify the original bytes without overwriting the live Syncthing tree.
- With `/var/lib/data` absent or replaced by the wrong mount, convergence and
  the timer fail closed without writing snapshots to the underlying `/var`.
  Record commands, guest state, and limitations in `butane/ACCEPTANCE.md`.

Local snapshots share the source disk's failure domain; this milestone does not
claim independent backup. Before cutover, inspect whether any real Syncthing
folders are nested Btrfs subvolumes and list each such source explicitly.
