# Clamps i915 boot crash investigation

Observed after the normal reboot/power-cycle on 2026-11-11:
`7.2.11-200.fc44.x86_64`, Intel i915 divide error at
`skl_compute_wm_params+0x21a/0x3a0`, called from `skl_cursor_allocation`,
`intel_atomic_check` and `intel_initial_commit` during driver probe.
The faulting instruction divides by R12D, which was zero. The exact source
operand needs the matching Fedora debuginfo/disassembly before attribution.
This is evidence of a display-driver initialization defect, not proof of why
the prior reboot failed to return. The previous journal ended normally during
systemd-shutdown; the post-journal shutdown/firmware interval is unobserved.

Next checks:
- Preserve kernel trace, firmware versions, connector/EDID state and pstore
  before changing graphics configuration. Privileged pstore access is pending.
- Resolve the exact installed Fedora source/debuginfo revision and map the
  faulting division. Upstream source searches found the watermark/cursor path
  but no verified matching fix or release to recommend.
- Reproduce warm reboot with console access; distinguish a display-only failure
  from loss of SSH/network. No additional reboot is authorized by this plan.
- Evaluate a newer supported kernel or a narrowly scoped display workaround
  only after matching evidence. Do not blacklist i915 blindly: that can remove
  console and hardware acceleration. Keep TPM recovery available for boot changes.

Reference source:
https://kernel.googlesource.com/pub/scm/linux/kernel/git/torvalds/linux/+/e1c04510f521e853019afeca2a5991a5ef8d6a5b/drivers/gpu/drm/i915/display/skl_watermark.c

## 2026-11-13 — repeated physical kexec hang

Operator reports another kexec attempt required forced power-off. Read-only SSH
inspection confirms staged-deployment finalization succeeded and logind accepted
kexec at 16:35:29. Systemd reached kexec.target and the journal stopped during
systemd-shutdown at 16:35:53. There is no persisted journal boot between that
attempt and the power-cycle boot at 16:47:28. This does not distinguish late
shutdown, the kernel handoff, early kernel initialization or initramfs unlock.

The recovered system boots signed uCore with Skillet 0.1.11, kernel
7.2.11-200.fc44.x86_64 and no failed units. This boot has i915 VBT/port warnings
at print_ddi_port, but not the earlier observed divide exception. The pstore
archival service skipped because the platform store was empty. No causal link
between those warnings and the hang has been established.

Unattended physical activation is not reliable acceptance. Pause activation
while preserving OS staging; the documented runtime mask clears at the next
boot. Remote sudo currently requires operator authentication, so no mask or
additional reboot was performed. Next supervised investigation must capture the
post-journal interval and compare ordinary reboot with kexec; previous ordinary
reboot failures prevent assuming that switching methods solves the problem.
Do not initiate further reboots or change graphics/TPM settings without an
explicit supervised test scope.
