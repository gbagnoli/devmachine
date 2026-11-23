# Automatic recovery from failed OS boots

Status: planned follow-up, 2026-11-23. Resolve the current Fedora 45 TPM unlock
failure separately; do not enable recovery or initiate production reboots as
part of documenting this plan.

## Goal and boundary

An unattended update must not leave a host waiting indefinitely for encrypted
root. Trial the candidate with a bounded attempt budget, then boot the retained
known-good deployment without an operator. Preserve Secure Boot, PCR-bound
unlock and independent recovery credentials; never auto-enroll an unapproved
PCR state or fall back to plaintext storage.

Normal Skillet/health services cannot recover a root-unlock failure because
those executables live on the locked volume. Recovery needs both an initramfs
failure deadline/reset and bootloader fallback state accessible before unlock.
A watchdog reset alone would otherwise boot the same failing image again.

## Implementation order

1. **Inspect the actual boot chain.** Verify CoreOS/bootupd GRUB configuration,
   writable boot-counter storage, deployment identity and ordering, and current
   success-marker writers. Evaluate packaged Greenboot (the maintained Rust
   implementation) for this rpm-ostree setup before adding custom orchestration;
   documentation for bootc or another Fedora edition is not compatibility proof.
2. **Bound the trial in initramfs.** Initially target a three-minute unlock/boot
   deadline and two candidate attempts. Verify systemd-cryptsetup/Clevis and
   dracut behavior together: a password prompt must not run forever. Dracut's
   emergency reboot settings alone do not prove the timeout reaches that path.
   Bake required early-boot support into the image/initramfs; a runtime unit or
   container on encrypted root cannot supply it. Keep an explicit supervised
   rescue mode for manual passphrase entry and diagnostics.
3. **Coordinate staging with fallback.** Retain an identified known-good
   deployment, arm the candidate attempt budget before activation, and decrement
   it in the bootloader before handing off to Linux. After exhaustion, select
   the known-good entry. Avoid relying on a mutable menu index without validating
   its deployment mapping. Make interrupted staging/finalization and repeated
   attempts idempotent; boot state must survive power loss with a safe fallback.
4. **Commit success or recover.** Mark success only after root/data and basic
   management readiness, not merely a login or a timer. Keep checks local and
   bounded; third-party APIs and individual application failures must not cause
   OS rollback loops. On fallback, reconcile the default to the working
   deployment, quarantine the failed image digest, suspend its automatic retry,
   and report recovery through existing monitoring. An explicit operator action
   or reviewed new candidate clears quarantine. Bound behavior when both entries
   fail; retain console rescue rather than looping forever.
5. **Add hardware hang coverage separately.** Clamps exposes `intel_oc_wdt` and
   `iTCO_wdt`; availability is not proof of a working reset. Validate the selected
   device, early-boot arming and handoff. PID 1 may keep feeding a watchdog while
   waiting for a password, so watchdog support cannot replace the unlock deadline.

## Acceptance before production activation

Use a named disposable UEFI/Secure Boot/swtpm VM with encrypted Btrfs root and
an independently tested recovery key. Record evidence in
[acceptance](archive/ACCEPTANCE.md).

- Healthy candidate: automatic unlock, management readiness and durable success;
  subsequent cold boot retains the accepted deployment.
- Deliberate candidate-only PCR mismatch: no console/password intervention;
  deadline causes reset, attempt budget expires, known-good root unlocks and SSH
  returns. Prove the entire path, not just a simulated unhealthy userspace unit.
- Failure after unlock and before success: bounded reset and working fallback.
- Interrupted staging/arming/success recording and brute-force power-off: retain
  a bootable known-good deployment and consistent retry state.
- Repeated update cycles: a quarantined digest is not silently retried by the
  daily update service; a new approved candidate can be trialed independently.
- Both-deployments-fail, offline boot and supervised rescue: finite behavior and
  preserved independent recovery. No automatic rebinding or secret disclosure.
- Hardware-watchdog reset and actual Clamps fallback require separate supervised
  physical acceptance after the VM tests; a software watchdog is not equivalent.

## References

- [Greenboot Rust implementation](https://github.com/fedora-iot/greenboot-rs)
- [Dracut early-boot timeout/emergency settings](https://github.com/dracut-ng/dracut/blob/main/man/dracut.cmdline.7.adoc)
- [TPM acceptance](TPM-ENCRYPTED-ROOT.md)
- [Image/update ownership](IMAGE-OWNED-CONFIGURATION.md)
