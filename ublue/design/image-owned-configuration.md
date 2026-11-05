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
The operator prefers kexec directly: the native VM mechanism passes, while
production activation ordering and supervised hardware acceptance remain required.
Retain an explicit full firmware reboot method.
The opt-in activation service now coordinates staged activation with cooperative
Skillet writers and the existing staging daemon. It uses native OSTree locking
and private mount access to finalize the selected deployment. Once `/etc` is
finalized, a boot-local gate refuses further managed writes until reboot, even
after worker failure; unloading the kernel alone cannot undo that merge boundary.
Cleanup restores the staging daemon's temporary mask. Other administrative tools
must observe the maintenance boundary. Remote multi-phase apply may be interrupted
and retried after reboot; its complete sequence is not an atomic transaction.
Firmware/TPM policy changes remain supervised. Existing runtime deployment and
container updates continue until their individual migrations are implemented.

[Implementation slices and acceptance](../plan/IMAGE-OWNED-CONFIGURATION.md).
