# Clamps VM acceptance log

Status: clean disposable VM create, readiness, and the real Skillet smoke
scenario passed on 2026-09-26. A subsequent fresh VM reached the signed
deployment and installed the full user environment. Bootstrap interruption
recovery remains unverified.

## Shared bootstrap and user environment

- The host-independent `ucore-bootstrap.service` read clamps' image reference
  from `/etc/ucore-bootstrap-image` and completed both rebases on a fresh VM.
- `brew-install.service` and `dotfiles-install.service` completed with status
  0. The `core` Brewfile bundle check passed, and the Brewfile and SSH key
  symlinks resolve into the dotfiles checkout. Completion markers survive
  reboot and allow an interrupted installation to retry.
- The first readiness run failed only because its symlink check compared a
  resolved `/var/home` path with an unresolved `/home` path. After correcting
  the comparison, `test-vm clamps ready smoke` passed on that same VM.
- `test-vm clamps reboot smoke` followed by `test-vm clamps ready smoke` passed; the
  temporary SSH key and completion markers survived the reboot.
- The VM interface is now `test-vm <host> <command> <instance>`, with `list`
  also accepting a host or listing all hosts. The Skillet wrapper accepts
  host and instance for `test vm create` and `test vm destroy`.
  `test-vm clamps status smoke` confirmed the retained domain. `test-vm list`
  showed clamps as available, smoke as running, a prior incomplete run as
  invalid, and beezelbot as unavailable because its Butane file is absent.
  Other hosts still need their own Butane configuration and host artifact.
- A second fresh VM, `clamps-test-generic` on port 2202, was created with the
  generalized command and passed signed-boot and full user-environment
  readiness.

## Passing clean run

- `cargo run --release -p skillet -- test vm create` created
  `clamps-test-smoke` from a fresh FCOS qcow2 and returned success without
  manual guest repair. The VM remains available for smoke runs.
- `runs/clamps-test-smoke/final-status.json` records the booted origin as
  `ostree-image-signed:docker://ghcr.io/gbagnoli/ucore-clamps:latest`; the
  previous deployment is the unsigned clamps image.
- The readiness helper verified the generated SSH key, guest binary SHA, successful
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
  the readiness helper transfers it over SSH after first boot. The clean create
  recorded above validated this change.

## Static checks

- ShellCheck passes for the four launcher/readiness scripts, bootstrap
  regression script and both Skillet smoke scripts.
- Full strict Butane compilation passed on 2026-09-26 with an earlier staged
  source and rebuilt host artifact. Local output:
  `/tmp/clamps-commit-validation-93upz2et/ignition/clamps.ign`.
- `bash tests/bootstrap.sh` passes eight mocked deployment-state cases:
  fresh clamps and bender boots, unsigned boot with signed rollback, signed pending, signed
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
| Repeat apply | Pass | `test-vm clamps ready smoke` reran the base unit with `systemctl --wait` |
| Reboot | Pass | Smoke reboot preserved data and container state |
| Interruption | Not run | Interrupt an isolated bootstrap, then recover |
| Repeatability | Not run | Second fresh VM after first passes |
| Real Skillet scenario | Pass | `skillet test smoke clamps` returned success |

The passing VM is retained for further inspection. The mutable
source-tree helper has not been installed as a frozen, persistently approved
host tool; it still uses normal command approvals.

## Pi-hole credential slice, 2026-09-27

- Fresh `clamps-test-pihole` VM on port 2202 passed `test vm create clamps
  pihole`, including unsigned/signed image rebases and base readiness. Its
  ignored `runs/clamps-test-pihole/` directory retains build and VM evidence.
- `test vm provision clamps pihole` delivered a generated dummy password via
  SSH stdin. The guest installed mode 0600, root-owned encrypted credential
  and loaded it through `skillet-full-apply.service`. A comparison of decrypted
  credential bytes with the live container's `/run/secrets/pihole_web_password`
  passed without printing the value.
- First full apply exposed a pre-existing `useradd` same-name group bug and a
  non-root Pi-hole startup failure. Both were fixed. `test vm update clamps
  pihole` deployed the current host binary while retaining the creation
  snapshot. Pi-hole then stayed active.
- Repeat provision kept container ID
  `de3716fc0a698771f792a1c909cec4f5486bbbe15f5fb74292da5a4b94c05e8c`.
  Explicit `--rotate` changed it to
  `dc4a8970644a44684b0d00b0f7dbf0d9bd242848b3a3ab2ff7b46bff94c55e87`,
  and the mounted secret matched the new credential. A reboot brought Pi-hole
  back active with the encrypted file intact.
- `skillet test smoke clamps --port 2202 --identity
  ../butane/runs/clamps-test-pihole/ssh/id_ed25519` passed its real-systemd
  fixture, including its own reboot. Evidence is under
  `/var/lib/skillet-smoke/` on the retained VM.
- Final workspace checks passed: 32 Rust tests, pedantic Clippy with warnings
  denied, `shellcheck` and `bash -n` for `bin/test-vm`, and `git diff --check`.
- Post-delivery OS rebase, interruption recovery, production TPM binding,
  Cloudflare token migration, and Pi-hole DNS/web behavior were not checked.

## Fresh default smoke VM, 2026-09-27

- The retained `clamps-test-smoke` VM predated `skillet-full-apply.service`:
  readiness passed, but `test vm provision clamps smoke` failed because that
  unit was absent. The owned disposable domain and run directory were removed.
- `test vm create clamps smoke` then built a fresh VM from current Butane and
  passed signed-image readiness and the user-environment checks. The run
  artifacts remain in the ignored `runs/clamps-test-smoke/` directory.
- `test vm provision clamps smoke` delivered a generated dummy credential and
  completed full apply. `test smoke clamps` passed all real-systemd/Podman
  fixture cases, including repeat apply, recovery, secret rotation, and reboot
  persistence. Guest evidence remains under `/var/lib/skillet-smoke/`.
- This run used no KeePassXC vault or Cloudflare token. It did not check
  storage mounts, Pi-hole DNS/web behavior, or post-delivery OS rebase.

## Shared Btrfs data storage, 2026-09-27

- Fresh `clamps-test-storage` VM on port 2203 booted from Butane with the
  shared `data-storage.bu` include. After the signed uCore bootstrap,
  `/var/lib/data` mounted as Btrfs `/data` on the same filesystem as `/var`.
  `skillet-data-prepare.service` created the `containers` subvolume and
  restored the rootful graphroot SELinux label. Podman reported graphroot
  `/var/lib/data/containers/system`, runroot `/run/containers/storage`, and
  driver `overlay`.
- Dummy-credential provisioning created the `pihole` child subvolume and
  started Pi-hole with bind paths under `/var/lib/data/pihole/{etc,dnsmasq.d,log}`.
  Repeated full apply retained the subvolume and running container. After
  Pi-hole changed ownership of its own files, two further applies made no
  metadata changes or container replacement.
- A sentinel in the Pi-hole data tree survived VM reboot. Temporarily masking
  `var-lib-data.mount` caused full apply to fail without creating fallback
  Pi-hole or container paths. A wrong tmpfs mount was rejected by both the
  shared preparation script and Skillet before either wrote to it. Restoring
  the real mount and running normal convergence recovered Pi-hole and retained
  the sentinel. The temporary mount override was removed.
- The real Podman/systemd `test smoke clamps` scenario passed against this VM,
  including its reboot and recovery cases, after deployment of the final
  shared full-apply guard. Workspace tests, pedantic Clippy, ShellCheck for
  both storage scripts, and `git diff --check` passed. These checks did not include a
  post-provision OS rebase or the physical stock SSD. Other host templates do
  not yet merge the shared include because they do not yet exist.
