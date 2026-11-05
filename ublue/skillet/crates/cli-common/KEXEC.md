# Controlled kexec service

On a selected host, using its installed host binary:

```sh
sudo skillet-clamps kexec install
sudo systemd-analyze verify /etc/systemd/system/skillet-kexec-activate.service
sudo systemctl start skillet-kexec-activate.service
```

Installation creates the service without enabling a schedule. Start requests a
reboot. Keep console/recovery access for initial physical acceptance.

The current service supports rebooting the booted deployment only. It refuses a
pending deployment, an already loaded kernel, or an unresolved ownership marker.
Do not schedule it for OS-update activation yet: coordinated staged finalization
and configuration exclusion remain pending in the
[image configuration plan](../../../plan/IMAGE-OWNED-CONFIGURATION.md).

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
