# Local UPS protection

Decision, 2026-10-12: install Fedora `nut` and `nut-client` in the shared
uCore image, with their services/enumerator disabled by default. An optional
shared Skillet module owns `/etc/ups`, USB selection and native service
activation. Clamps selects the CyberPower CP1500 AVR (`0764:0501`) with
`usbhid-ups`; other host callers must explicitly opt in.

Use `MODE=standalone`, with `upsd` listening only on IPv4/IPv6 loopback.
A generated, retained host-local monitor password grants only `upsmon primary`.
It needs no vault entry or operator login; root-only state and root:nut
configuration retain it through rebases and interrupted apply. Never silently
replace malformed retained credentials. This is local service authentication,
not a portable operator credential.

Use the packaged driver/server/monitor and late shutdown hook. `upsmon` powers
off the host on critical battery state, not on every brief outage. Its power-down
flag lives under `/run/nut` so an old flag cannot persist across reboots. Actual
UPS turn-off delays and firmware restart-after-power-loss need hardware acceptance.

The generic USB resource inspects sysfs and refreshes scoped udev rules.
A matching USB device gets a stable alias and starts the monitor dependency
chain on hotplug. Unit conditions skip absent hardware; configuration still
converges on ordinary smoke VMs. Losing the cable must not stop an already
running monitor: its last known on-battery state can require protective shutdown.
Datadog unit monitoring is selected only when the UPS is attached at agent apply.
Removing the declaration leaves existing resources unmanaged; explicit removal
is required to disable previously configured protection.

Validate configuration with real Fedora NUT tools and a simulated driver whose
shutdown command is harmless. USB polling, real battery shutdown and UPS load
cycling remain physical acceptance; never invoke forced shutdown during routine
smoke tests. See [acceptance](../butane/ACCEPTANCE.md).
