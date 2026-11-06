# Public image bundles

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
are rejected. The output contains only two executables and their public release
manifest under `usr/`; it needs no vault, SSH, running services or storage mounts.

An image builder can copy the bundle contents into `/`. Preserve modes when
transporting it: archive the tree rather than uploading individual executables.
To verify a transported tree, run its generic executable with the same command,
using the transported binary paths and its existing root as `--output`.

Existing `/var/usrlocal/bin` delivery and unit references remain active. Inspect
the candidate binaries explicitly as `/usr/bin/skillet --version` and
`/usr/bin/skillet-clamps --version` after building an image. No boot consumer is
switched by this bundle. Service definitions and precedence migration are pending.

Ownership: [inventory](../../../../plan/IMAGE-OWNERSHIP-INVENTORY.md),
[design](../../../../design/image-owned-configuration.md).
