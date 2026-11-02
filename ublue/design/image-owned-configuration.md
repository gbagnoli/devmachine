# Direction: image-owned service configuration

Planned after Clamps production acceptance; not the current deployment behavior.
Signed host releases will bind Skillet binaries, public static configuration and
pinned container digests. Reuse canonical declarations and pure rendering for
build output; do not run machine-observing apply during image construction.

Runtime retains private inputs/credentials, provider enrollment, actual storage
preparation, first-use initialization and application data. Builds must need no
vault, production host or private values. Vendor files live under `/usr` where
supported, with explicit migration of earlier `/etc` and mutable binary overrides.
Each output has one owner. Image rollback never implies application-data rollback.

Move service updates from independent mutable-tag pulls to tested digest updates
and signed release promotion. The eventual activation policy is daily normal
reboots in staggered host windows; keep scheduling disabled until accepted.
Kexec remains an optional separately tested optimization, not a prerequisite.
Firmware/TPM policy changes remain supervised. Existing runtime deployment and
container updates continue until their individual migrations are implemented.

[Implementation slices and acceptance](../plan/IMAGE-OWNED-CONFIGURATION.md).
