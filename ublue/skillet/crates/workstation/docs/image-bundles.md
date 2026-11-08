# Public image bundles

ucore-images selects a tagged source release through its version/commit lock.
See [release and dependency updates](releases.md).

Build matching binaries from a clean, exact source checkout:

```sh
export PATH=/opt/x86_64-linux-musl-cross/bin:$PATH
cargo build --locked --release -p skillet -p skillet-clamps
revision=$(git rev-parse HEAD)
target/x86_64-unknown-linux-musl/release/skillet image clamps \
  --source-revision "$revision" \
  --generic-binary target/x86_64-unknown-linux-musl/release/skillet \
  --host-binary target/x86_64-unknown-linux-musl/release/skillet-clamps \
  --output /tmp/clamps-image-bundle
```

The output parent must exist. Identical repeats are accepted; changed content,
extra files, symlinks or unexpected permissions are refused. Both binaries must
report the same clean source version and be static x86-64 ELF files. Dirty builds
are rejected. The output contains two executables, caller-selected btrbk vendor
config/units/ownership marker, kexec/HA exporter and freshness/NUT collector units,
disabled optional-unit presets, and a public release manifest under `usr/`.
The manifest covers every output's hash and mode. Rendering needs no vault,
SSH, running services or storage mounts.

An image builder can copy the bundle contents into `/`. Preserve modes when
transporting it: archive the tree rather than uploading individual executables,
and extract with `tar --same-permissions --no-same-owner` so the receiving umask
does not remove executable bits. Create the receiving output root with
`install -d -m 0755` before extraction; all bundle directories must be `0755`.
To verify a transported tree, run its generic executable with the same command,
using the transported binary paths and its existing root as `--output`.

Existing `/var/usrlocal/bin` delivery and unit references remain active. Inspect
the candidate binaries explicitly as `/usr/bin/skillet --version` and
`/usr/bin/skillet-clamps --version` after building an image. No boot consumer is
switched for general apply by this bundle. After module reconciliation, btrbk's
vendor service uses the image caller binary. See the
[btrbk migration guide](../../btrbk/README.md) before retiring legacy overrides;
retain a migration-capable mutable caller for OS rollback.

Ownership: [inventory](../../../../plan/IMAGE-OWNERSHIP-INVENTORY.md),
[design](../../../../design/image-owned-configuration.md).

## Optional vendor unit activation

Kexec activation, HA backup export/freshness and NUT collector definitions are
included on every host image. Their timers default to disabled; canonical host
selection and credential-gated apply activate them. They execute `/usr/bin/skillet`.
Existing `/etc` overrides are archived only after exact-byte/receipt validation;
unknown edits or partial/mismatched image definitions stop reconciliation.
Older images keep the legacy installer until these vendor units are available.
Publish the new Skillet release, update the ucore-images dependency lock, rebuild
and boot that image, then apply the matching release to migrate retained overrides.
Do not remove unknown `/etc` files manually to force activation.

For scoped systemd ownership acceptance, create a separate base-only disposable
VM and run the fixture from `ublue/skillet`:

```sh
cargo run --release -p skillet -- test vm create clamps image-units --port 2209
cargo run -p skillet_workstation --example vendor_units_acceptance -- ../butane clamps image-units
cargo run --release -p skillet -- test vm destroy clamps image-units
```

The fixture checks manifest/domain ownership and the guest hostname. It uses a
temporary `/usr` overlay, replaces action commands with harmless runtime overrides,
and checks legacy migration,
repeat apply, custom-edit refusal and effective vendor fragments with the real
systemd parser. It stops and disables fixture timers before returning. It needs no vault or provider
access; it does not establish image publication/signatures or reboot persistence.
Destroy this dedicated VM afterwards rather than using it for application work.
