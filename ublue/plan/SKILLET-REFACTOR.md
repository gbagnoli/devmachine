# Refactor remaining acceptance

Implementation is complete. Preserve the following open checks; this document
is not evidence that they passed. Archived workstreams and follow-ups retain
original scope and results in [the archive](archive/README.md).

## Remaining checks

1. **Parked Flatpak/Syncthing acceptance:** when that backend is available, use
   an owned named VM to repeat application apply and reboot after the committed
   host/container ownership mapping correction. Verify mounts, numeric/named
   ownership, unchanged managed metadata and persistent data. Previous native
   acceptance does not establish Flatpak equivalence.
2. **Remote CI evidence:** observe checks for the relevant pushed revision;
   local formatting, Clippy, workspace tests and container checks do not prove
   a remote run passed. Record the revision/run/result separately.
3. **Standalone Butane artifact lookup:** the compiler wrapper currently builds
   static binaries then uses the configured target directory/conventional musl
   layout. Audit migration to the shared Cargo JSON build-result owner; do not
   describe that wrapper as already selecting Cargo-reported artifacts.

## Completion

Use the [VM guide](../skillet/crates/vm/README.md) and shared ownership transport.
Record actual evidence in [acceptance](archive/ACCEPTANCE.md). Do not substitute
fixture-only smoke for application or backend acceptance. Parked checks remain
unverified while independent user-authorized work proceeds.

Architecture rules remain in [AGENTS](../AGENTS.md) and
[the design](../design/skillet-architecture.md). Production renewal, real-client
network access and cutover belong to their service/migration plans.
