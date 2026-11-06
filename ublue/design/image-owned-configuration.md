# Direction: image-owned service configuration

btrbk has an image-owned pilot with scoped native VM acceptance. Other services
retain runtime ownership; published-image and physical migration acceptance remain
explicit gates for changing their owners.
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
The `skillet_kexec` crate owns activation convergence and guest runtime behavior.
Hosts opt in through their canonical service declarations; Clamps declares it.
Base and full apply install the unit without starting it or enabling a timer.
Definition convergence records a successful systemd reload so interrupted
installation retries the reload. No separate installation command is needed.
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

Image bundles are assembled from supplied clean, matching generic/caller binaries
and the canonical public service declaration. Output includes `/usr/bin`
binaries, selected btrbk vendor artifacts and `/usr/share/skillet/release.json`
recording exact source revision and all artifact hashes/modes.
No apply is executed to discover output, and no private input
is accepted. CI consumes a full source commit rather than a moving branch; the
receipt identifies actual binary bytes without promising bit-identical rebuilds.
Mutable `/var/usrlocal` references retain operational ownership until a separately
tested precedence/override migration; candidate image artifacts do not silently
replace those consumers. btrbk is the first static-service pilot: its verified
migration switches the snapshot service to the image caller, while general apply
and bootstrap references retain their existing owners.
