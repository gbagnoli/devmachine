# Butane and installation inputs

[Physical installation](HARDWARE.md) and [disposable VM operation](../skillet/crates/vm/README.md)
have separate guides. `clamps.bu` and `joey.bu` are **VM templates** using
`/dev/vda`; never pass them directly to an installer for a physical disk.

## Compile

From `ublue/butane`, the standalone compiler is:

```sh
./bin/butane clamps.bu
```

Use `./bin/butane -h` for output/options. When a source includes
`includes/skillet.bu`, it builds/stages static generic and host binaries unless files were already
supplied. `CARGO_TARGET_DIR` redirects build
output. The normal destination Ignition contains both binaries; VM creation
transfers them over verified SSH after boot instead of embedding large binaries
in QEMU fw_cfg.

## Shared inputs

- `includes/skillet.bu`: generic binary and credential-gated apply units.
- `includes/data-storage.bu`: Btrfs data mount, graphroot and mount prerequisites.
- uCore bootstrap reads `/etc/ucore-bootstrap-image`, completes unsigned then
  signed rebase, and gates base apply on the **booted** signed deployment.
- `/etc/skillet/host` selects the stable caller independently of runtime hostname.

Physical media generation selects an explicit whole-disk by-id destination and
root profile; its live helper verifies inputs and requires server-side destructive
confirmation. Follow [HARDWARE.md](HARDWARE.md) for access material, USB writing,
MOK enrollment and provisioning. The VM `/dev/vda` path remains independent.

## Helpers and regressions

`bin/test-vm HOST COMMAND INSTANCE` is an argument adapter for the Rust VM owner.
Use the [VM guide](../skillet/crates/vm/README.md) for backend prerequisites,
readiness, inspection, updates, forwards and disposal. Other virt wrappers use
existing native/Flatpak tools; helpers do not install workstation packages.

From this directory, run bootstrap state regressions with `bash tests/bootstrap.sh`.
They use mocked deployment states and do not modify a VM. Isolated UniFi fence
fixtures under `tests/fixtures/unifi-restore/` belong only to disposable restore
checks; never merge them into production configuration.

Designs: [storage](../design/storage.md), [VM ownership](../design/smoke-vms.md).
