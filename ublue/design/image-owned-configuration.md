# Direction: image-owned service configuration

btrbk has an image-owned pilot with scoped native VM acceptance. Kexec activation,
HA backup/export freshness and NUT collector definitions now join the image bundle;
published-image and physical migration acceptance remain
explicit gates for changing their owners.
Signed host releases bind Skillet binaries and public static configuration.
Application image pinning remains deferred. Reuse canonical declarations and pure rendering for
build output; do not run machine-observing apply during image construction.

Runtime retains private inputs/credentials, provider enrollment, actual storage
preparation, first-use initialization and application data. Builds must need no
vault, production host or private values. Vendor files live under `/usr` where
supported, with explicit migration of earlier `/etc` and mutable binary overrides.
Each output has one owner. Image rollback never implies application-data rollback.

Applications currently retain moving stable/latest channels and registry updates;
digest-based release promotion is deferred. Static unit ownership can move
independently. Clamps uses daily kexec at 05:30 Europe/Paris and a normal firmware
reboot on the first Sunday of each month. Supervised physical staged-image kexec
passed on 2026-11-08 with an unchanged kernel version; different-kernel physical
acceptance remains separate. Retain an explicit full firmware reboot method.
The `skillet_kexec` crate owns activation convergence and guest runtime behavior.
Hosts opt in through their canonical service declarations; Clamps declares it.
Base and full apply reconcile the definitions and enable only selected host timers.
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
binaries, selected btrbk vendor artifacts, optional maintenance/collector vendor
units with disabled presets, and `/usr/share/skillet/release.json`
recording exact source revision and all artifact hashes/modes.
No apply is executed to discover output, and no private input
is accepted. CI consumes a full source commit rather than a moving branch; the
receipt identifies actual binary bytes without promising bit-identical rebuilds.
The version/commit dependency lock and published source releases follow the
[Skillet release contract](skillet-releases.md).
Mutable `/var/usrlocal` references retain operational ownership until a separately
tested precedence/override migration; candidate image artifacts do not silently
replace those consumers. btrbk is the first static-service pilot: its verified
migration switches the snapshot service to the image caller, while general apply
and bootstrap references retain their existing owners.

Kexec, HA backup/export freshness and NUT collectors share pure unit rendering
between release assembly and runtime ownership validation. Every host image may
ship their disabled definitions; caller opt-in and credential-gated monitoring
still own activation. Runtime never rewrites vendor files. Existing images retain
the legacy installer until the vendor group appears. Partial or mismatched groups
fail closed, so image and delivered runtime definitions must agree. Known generated
`/etc` definitions are archived after complete preflight, with legacy receipts
validated for kexec/NUT. Old HA installers had no receipt; exact known generated
bytes identify their overrides. Unknown edits require explicit inspection.
NUT's UPS selection/socket remain in a root-only runtime environment file.
Runtime apply/credential delivery and unrelated application units keep their owners.
