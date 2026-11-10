# Controlled kexec service

Opt in through the canonical host profile with
`HostService { config: ServiceConfig::Kexec, ui: None }`. Clamps declares this;
Joey and Donbot currently do not. The host binary links `skillet_kexec` and the
shared profile runner calls its `service::apply` in both base and full apply.
The shared CLI delegates worker/cleanup commands to the same crate.

Deploy and apply normally:

```sh
cargo run --release -p skillet -- apply clamps
```

Image bundles ship the activation/reboot services and maintenance timers under
`/usr/lib/systemd/system`, disabled by default. Apply verifies image definitions,
retires known runtime overrides and enables the selected host timers without
starting a reboot. Older images retain the legacy installer until upgraded.
There is no separate `kexec install` step.

For a supervised activation on the host:

```sh
sudo systemd-analyze verify skillet-kexec-activate.service
sudo systemctl start skillet-kexec-activate.service
```

Starting the service requests a reboot. Keep console/recovery access for initial
physical acceptance.

The service selects the current pending deployment, or the booted deployment when
none is pending. It uses OSTree's native OFD sysroot lock to check for a concurrent
transaction, then runtime-masks/stops the existing staging daemon before finalizing.
It uses a private mount namespace for writable sysroot access. Cleanup restores
its temporary daemon mask; normal reboot also clears runtime masks.

Skillet apply, credential installation, SMTP preparation and access finalization
participate in shared configuration exclusion. Activation takes exclusive exclusion.
After staged `/etc` finalization, a `/run` gate refuses further managed writes until
reboot, even if the activation worker fails. A failed handoff unloads the kernel,
but operators must reboot before resuming configuration. Do not bypass the gate or
edit `/etc` manually during activation. Other root tools are outside this cooperative
protocol. A remote multi-phase apply interrupted between phases must be retried
following reboot; it is not an atomic transaction.

OS kexec activation follows successful completion of
`rpm-ostreed-automatic.service`, after it has staged an update. With no staged
deployment, activation exits without rebooting on ordinary days. On the first
Sunday of each month, the same staging completion requests a normal reboot even
when no update was pending. Both the old daily kexec timer and the separate monthly
firmware-reboot timer are disabled, so there is one activation after staging.
Logged-in users are detected before deployment finalization, so activation safely
defers and the staged update remains available for a later automatic staging run.
If a login or other inhibitor races with the final kexec request after finalization,
the next run can reuse the finalized default deployment. An active HA export or
btrbk job also refuses activation before deployment finalization.
Monitor uptime over seven days and host availability.
The rpm-ostreed staging timer currently runs around 06:30 Europe/Paris; kexec
starts after successful staging on ordinary days; the first Sunday takes the
normal reboot path after that staging run. The exporter runs at 04:00 and btrbk
on the hour. The idle check does not prevent an independently started backup
from beginning afterwards.

Inspect schedules with `systemctl list-timers --all 'skillet-*reboot*' 'skillet-kexec*'`.
To pause update-triggered activation temporarily, use
`sudo systemctl mask --runtime --now skillet-kexec-activate.service`; unmask it
to resume. This pauses both update activation and the monthly reboot. A pending
update remains staged and is retried after activation resumes.
[reboot policy](../../../plan/IMAGE-OWNED-CONFIGURATION.md).

The service loads the uniquely matching signed kernel and initramfs, then requests
graceful kexec respecting shutdown inhibitors. Its independent `ExecStopPost`
unloads only a kernel associated with its own boot and invocation. A 120-second
runtime limit cleans up an abandoned handoff. Cleanup failures remain visible in
the journal and retain the ownership marker, preventing another attempt.

The ownership marker is under `/run`; neither it nor an armed kernel survives a
power loss. The ordinary OSTree bootloader entries remain available for firmware
boot. Rebooting does not restore application data changed by an OS update.

Inspect failures with `journalctl -u skillet-kexec-activate.service`. Do not remove
an unresolved marker or unload a kernel until ownership has been investigated.
