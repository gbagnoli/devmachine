# Controlled kexec service

On a selected host, using its installed host binary:

```sh
sudo skillet-clamps kexec install
sudo systemd-analyze verify /etc/systemd/system/skillet-kexec-activate.service
sudo systemctl start skillet-kexec-activate.service
```

Installation creates the service without enabling a schedule. Start requests a
reboot. Keep console/recovery access for initial physical acceptance.

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

Scheduling remains disabled pending physical acceptance and the remaining
[reboot policy checks](../../../plan/IMAGE-OWNED-CONFIGURATION.md).

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
