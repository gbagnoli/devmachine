# Clamps VM acceptance log

Status: clean disposable VM create, readiness, and the real Skillet smoke
scenario passed on 2026-09-26. Bootstrap interruption recovery remains
unverified.

## Passing clean run

- `cargo run --release -p skillet -- test vm create` created
  `clamps-test-smoke` from a fresh FCOS qcow2 and returned success without
  manual guest repair. The VM remains available for smoke runs.
- `runs/clamps-test-smoke/final-status.json` records the booted origin as
  `ostree-image-signed:docker://ghcr.io/gbagnoli/ucore-clamps:latest`; the
  previous deployment is the unsigned clamps image.
- `clamps-ready` verified the generated SSH key, guest binary SHA, successful
  repeated base apply, enforcing SELinux, NetworkManager's resolver, working
  DNS, and masked `systemd-resolved`.
- Ignition keys survived under `~/.ssh/authorized_keys.d/ignition`. Skillet's
  SSH config now includes that path. The image repo has the matching change,
  which will take effect after its next image build.
- `cargo run --release -p skillet -- test smoke clamps` passed the real
  Podman/systemd fixture, repeated apply, config and secret changes, injected
  startup failures, and post-reboot persistence. Snapshots and journals remain
  under `/var/lib/skillet-smoke` in the disposable VM.

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
  a later clean VM create passed. Per-VM confinement was not
  disabled and host security policy was not changed.
- Failed generated artifacts remain in ignored `runs/` and `/tmp` paths for
  diagnosis. They contain private test SSH keys and should not be committed.
- A later `runs/clamps-test-0925-05` directory contains generated artifacts
  but no recorded domain UUID or acceptance results. This is not evidence of
  a successful guest boot. The helper now selects a separate native libvirt
  runtime to avoid mixing native and Flatpak daemon filesystem views; that
  approach passed the clean create recorded above.
- `runs/clamps-test-smoke` created a running VM, but SSH on port 2201 never
  became available. The serial console showed Ignition still reading the
  2.5 MiB `fw_cfg` payload after about 30 host minutes; the dominant
  embedded entry was the 4.9 MiB `skillet-clamps` binary. Ignition documents
  that large QEMU `fw_cfg` configs can take an unreasonable time to read
  ([release notes](https://coreos.github.io/ignition/release-notes/)). The VM
  launcher now omits that binary from the VM-only Ignition config and
  `clamps-ready` transfers it over SSH after first boot. The clean create
  recorded above validated this change.

## Static checks

- ShellCheck passes for the four launcher/readiness scripts, bootstrap
  regression script and both Skillet smoke scripts.
- Full strict Butane compilation passed on 2026-09-26 with an earlier staged
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
- Current VM Skillet artifact SHA256:
  `6cdb80f03221c0fb9521f7ff9a58ee33844fb7262651ff47793b9a9f70ec7784`.

## Exit criteria

| Criterion | Result | Evidence / next command |
| --- | --- | --- |
| Clean install | Pass | Clean `test vm create` returned success |
| Access | Pass | Generated key authenticated after signed boot |
| Rebase sequence | Pass | Booted signed and previous unsigned clamps deployments |
| Resolver continuity | Partial | Final boot passed; each intermediate boot not checked |
| Artifact delivery | Pass | Guest SHA matched `skillet.sha256` |
| Automatic apply | Pass | Boot unit completed with status 0 |
| Repeat apply | Pass | `clamps-ready` reran the base unit with `systemctl --wait` |
| Reboot | Pass | Smoke reboot preserved data and container state |
| Interruption | Not run | Interrupt an isolated bootstrap, then recover |
| Repeatability | Not run | Second fresh VM after first passes |
| Real Skillet scenario | Pass | `skillet test smoke clamps` returned success |

The passing VM is retained for further inspection. The mutable
source-tree helper has not been installed as a frozen, persistently approved
host tool; it still uses normal command approvals.
