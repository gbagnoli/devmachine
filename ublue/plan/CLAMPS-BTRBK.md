# Next milestone: btrbk snapshots on clamps

Status: implemented and verified on the retained disposable clamps VM. Scope is
local snapshot parity for rupik; remote backup and Pi-hole snapshot policy are
separate work.
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

1. Done in `ucore-images`: add `btrbk` to the shared OS recipe
   `recipes/ucore-common.yml` and mask its packaged daily timer. Build the
   common image before the host images as their CI requires. Do not install
   packages at runtime or copy Chef's source-build recipe.
2. Done in Skillet: add a reusable btrbk module and make hosts opt in with
   explicit paths relative to `/var/lib/data`. Clamps passes only `syncthing`;
   no host implicitly snapshots the root data subvolume or Podman containers.
   The module requires the expected data mount and an existing source
   subvolume, then manages retention/config and its timer idempotently.
3. Implemented: mask the package's daily timer in the shared image and manage
   one hourly Skillet systemd timer. The service runs the shared data mount
   validation before btrbk, and is bound to the mount.
   The service must verify the actual `/data` Btrfs mount on every run, using
   the same checks as `butane/includes/data-storage.bu`; a mountpoint assertion
   alone does not reject a wrong filesystem. Do not enable a second packaged
   timer or create a cron job. Apply the shared module from clamps' host
   convergence only after Syncthing storage exists.
4. Done: isolated unit tests cover caller-selected rendering, source path
   safety, opt-out, repeat apply, and fail-closed behavior for a wrong data
   mount.
5. Done: rebuilt a named disposable clamps VM from the image with btrbk,
   provisioned dummy credentials, and validated the real systemd timer and
   Btrfs subvolume.

## Exit criteria

- Shared `ucore-images` recipe contains btrbk. Workspace format, pedantic
  Clippy, unit tests, the CI container command, and the named VM smoke pass.
  The VM uses real systemd and Podman.
- `btrbk -n -v run` targets only the intended Syncthing subvolume; a real run
  creates a read-only snapshot under the intended directory. Repeated Skillet
  apply preserves the config, timer, and snapshots. After reboot the timer is
  active and enabled, and its next run is scheduled at the next hourly boundary.
- Put a disposable sentinel in the VM's Syncthing data, snapshot it, change
  the live file, and restore the snapshot into a **separate temporary path**.
  Verify the original bytes without overwriting the live Syncthing tree.
- With `/var/lib/data` absent or replaced by the wrong mount, convergence and
  the timer fail closed without writing snapshots to the underlying `/var`.
  The real wrong-mount and absent-mount checks are recorded in
  `butane/ACCEPTANCE.md` under shared Btrfs data storage.

Local snapshots share the source disk's failure domain; this milestone does not
claim independent backup. Nested subvolumes inside a selected snapshot source
are unsupported by design; Syncthing data sources must remain leaf subvolumes.

## Current verification

- Image workflow [run 36552978960](https://github.com/gbagnoli/ucore-images/actions/runs/36552978960)
  succeeded for the common image and all three host images. The published
  `ucore-clamps:latest` digest is
  `sha256:e6c0cd35641199ddb5e2d9ca0f281dc1d640126ae5b2a04fa87c65163cedfd7c`.
- The user created `clamps-test-smoke`; the current session can inspect its
  native `qemu:///session` domain. It booted the signed clamps image at the
  digest above. The guest confirms `btrbk-0.32.7-1.fc44.noarch` is installed,
  the package's `btrbk.timer` is masked, and `/var/lib/data` is mounted from
  Btrfs `/data` on the expected filesystem.
- The first readiness attempt exposed an incorrect inherited rule to pass
  `--wait` to systemctl. Because that option waits for the unit to stop,
  `systemd-sysctl.service` (`Type=oneshot`, `RemainAfterExit=yes`) hung. The
  rule and all Skillet and VM helper callers were corrected to use ordinary
  blocking `start`/`restart`; the updated binary completed base apply.
- Full apply created the Syncthing subvolume and configured the hourly timer.
  `btrbk -n -v run` selected only `/var/lib/data/syncthing`. A real run created
  `syncthing.20260929T1402`, reported `ro=true`, and a separate temporary
  subvolume restored the original sentinel bytes after the live sentinel was
  changed. The timer also ran at 14:00:30 and created
  `syncthing.20260929T1400`.
- Repeated full apply retained config/timer hashes and both snapshots. After
  reboot, Pi-hole and Syncthing were active, the Syncthing subvolume and both
  snapshots remained, `skillet-btrbk.timer` was active/enabled, and the
  packaged `btrbk.timer` remained masked; the next timer run was 15:00 CEST.
  `test smoke clamps` passed its real-systemd/Podman cases through reboot.
- Local validation after the correction: `cargo fmt --check`, all 45 workspace
  tests, pedantic Clippy with warnings denied, the CI-style container command
  (`cargo run --release -p skillet -- test run clamps --phase base --image
  fedora:latest`), ShellCheck and `bash -n` for `test-vm-ready`, and
  `git diff --check` passed. The container command ran from a writable
  temporary workspace copy because the checked-out workspace is mounted
  read-only to shell commands.
