# Clamps VM acceptance log

Status: integration incomplete, reviewed 2026-09-26. The current source has not yet
completed a fresh VM boot. Nothing below is claimed as a passing end-to-end
install.

## Preflight and failed attempts

- Native `virsh -c qemu:///session list --all` worked with host execution and
  showed no existing domains before provisioning. Host has `passt` and
  `virt-install`; a read-only XML preview showed loopback-bound passt forwarding.
- Initial launcher attempt `clamps-agent2-0925-01` stopped before VM creation
  on an invalid yq expression; fixed in `bin/butane`.
- `clamps-agent2-0925-02` compiled strict Butane output but QEMU refused to
  open the fw_cfg Ignition file in the workspace (`Permission denied`).
- `clamps-agent2-0925-03` produced the same QEMU error with artifacts under
  `/tmp`. No domain remained after either failed virt-install. The current
  source adds the Ignition file as a read-only libvirt-managed raw attachment;
  this change still needs a fresh VM attempt. Per-VM confinement was not
  disabled and host security policy was not changed.
- Failed generated artifacts remain in ignored `runs/` and `/tmp` paths for
  diagnosis. They contain private test SSH keys and should not be committed.
- A later `runs/clamps-test-0925-05` directory contains generated artifacts
  but no recorded domain UUID or acceptance results. This is not evidence of
  a successful guest boot. The helper now selects a separate native libvirt
  runtime to avoid mixing native and Flatpak daemon filesystem views; that
  approach still requires end-to-end validation.

## Static checks

- ShellCheck passes for the four launcher/readiness scripts, bootstrap
  regression script and both Skillet smoke scripts.
- Full strict Butane compilation passed on 2026-09-26 with the final staged
  source and rebuilt host artifact. Local output:
  `/tmp/clamps-commit-validation-93upz2et/ignition/clamps.ign`.
- `bash tests/bootstrap.sh` passes seven mocked deployment-state cases:
  fresh boot, unsigned boot with signed rollback, signed pending, signed
  booted (tag and digest), reboot retry limit, and rebase failure.
  These are regression checks, not a replacement for actual VM boots.
- Review corrected duplicate `:latest` matching, rollback selection as a
  pending deployment, and reuse of a stale default Skillet build. The default
  launcher now rebuilds; `--artifact` remains an explicit override.
- Skillet review passed formatting, 25 workspace tests, configured pedantic
  Clippy with warnings denied, and the offline static release build.
- Latest Skillet handoff artifact: `skillet-clamps` SHA256
  `55bf7584ce405dcf2c20956c182f4dbfe04bfbb20a9b649e48ab97877d9e8856`.
  No VM has consumed this final artifact yet.

## Exit criteria

| Criterion | Result | Evidence / next command |
| --- | --- | --- |
| Clean install | Fail | QEMU fw_cfg EACCES on attempts 02/03; retry with current source |
| Access | Not run | `bin/clamps-ready RUN_DIR` after guest boot |
| Rebase sequence | Not run | Guest `rpm-ostree status --json` and bootstrap journals |
| Resolver continuity | Not run | `bin/clamps-ready` DNS/link checks on each boot |
| Artifact delivery | Not run | Guest SHA versus `skillet.sha256` |
| Automatic apply | Not run | `systemctl status skillet-apply.service` |
| Repeat apply | Not run | Two base applies with state/service timestamps |
| Reboot | Not run | Ordinary VM reboot and repeated readiness checks |
| Interruption | Not run | Interrupt an isolated bootstrap, then recover |
| Repeatability | Not run | Second fresh VM after first passes |
| Real Skillet scenario | Not run | `skillet/integration_tests/smoke-ssh.sh` with VM identity |

No successful VM is established by the saved evidence. Next, create a new
`clamps-test-*` domain and retain a passing guest for acceptance. The mutable
source-tree helper has not been installed as a frozen, persistently approved
host tool; it still uses normal command approvals.
