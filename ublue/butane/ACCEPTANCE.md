# Clamps VM acceptance log

## Refactoring and disposable UI revalidation, 2026-10-04

- Passed `cargo fmt --all -- --check`, strict workspace Clippy, and
  `cargo test --workspace --all-targets --offline`. The CI Fedora repeat-apply
  command passed: `cargo run --offline --bin skillet -- test run beezelbot
  --phase base --image fedora:latest`.
- A fresh `clamps-test-generic-ui` VM was created on SSH port 2202 and reached
  `Ready` through signed uCore boot, bootstrap, and user-environment checks.
  `cargo run --offline --bin skillet -- test smoke clamps --instance
  generic-ui` passed its real systemd/Podman apply, failure-recovery, and
  reboot-persistence cases. `test vm destroy clamps generic-ui` removed the
  disposable VM and its recorded Tailscale device.
- Live provisioning exposed two fresh-host issues and they are fixed in code:
  staged Butane `mode` values are normalized to numeric values before compile,
  and Pi-hole credentials are delivered before Tailscale so the full-apply
  unit receives its complete credential prerequisites. Tailscale enrollment
  now treats a confirmed absent container as unenrolled while keeping other
  Podman probe failures fail-closed. Unit coverage and all local checks pass.
  A credential-empty fresh VM has not yet completed that entire enrollment
  sequence end-to-end; the live run reached Cloudflare after retrying a VM with
  partially delivered credentials.
- The latest `test vm provision clamps generic-ui --with-ui` run enrolled the
  VM in Tailscale but stopped before Cloudflare mutation because KeePassXC
  lacks `skillet/environments/test/dns/cloudflare-zone-id`. The disposable VM and
  its Tailscale device were cleaned up. Current live Caddy ACME/HTTPS
  acceptance is blocked until that shared zone entry is present; no production
  DNS or credentials were changed.

## Recoverable VM creation intent, 2026-10-03

- Creation now writes a mode-0600 Rust manifest with a generated domain UUID
  before installer staging. Repeated preparation resumes only the same
  `Preparing` intent with matching backend, runtime, SSH port, and source
  revision; ambiguous and progressed directories are refused.
- The UUID is embedded in native libvirt XML and passed to Flatpak `virt-install`.
  Native definition is recorded as `Defined` before start. Failed start no
  longer undefines the guest. Re-running create checks UUID and disk ownership,
  starts a verified stopped domain, or resumes staging when no domain exists.
  Captured hashes and `Started` are written after the helper succeeds.
- Local regression tests cover intent reuse/conflict, private run-directory
  permissions, and phase transitions. Passed `cargo fmt --all --check`,
  offline workspace all-target Clippy with `-D warnings`, offline workspace
  tests/build, the CI Fedora repeat-apply container integration, ShellCheck for
  both changed helpers, CLI help, and `git diff --check`. No VM, libvirt domain,
  vault, or provider resource was contacted for this change.

## Native XML rendering, 2026-10-03

- Native domain XML is rendered in `skillet_vm` with `quick-xml`, written
  atomically as a private file under the manifest's run directory, and consumed
  by the existing `virt-xml-validate` and `virsh define` steps. The renderer
  carries the manifest UUID, exact disk and Ignition paths, QEMU fw_cfg input,
  passt loopback SSH forwarding, serial console, and original machine profile.
  Attribute escaping is checked using a path containing `&`.
- Removed the Python XML generator and the Python 3 VM preflight requirement.
  Backend/emulator selection and subsequent define/start remain in Bash for now;
  Flatpak still delegates VM construction to `virt-install`.
- Passed format, offline workspace all-target Clippy (`-D warnings`), workspace
  tests (51 VM tests), build, ShellCheck for both changed helpers, and the CI
  Fedora repeat-apply container integration. The renderer test validates the
  generated XML against the same name, UUID and disk ownership parser used by
  status inspection. No VM/libvirt domain was contacted.

## Rust readiness migration, 2026-10-03

- The CLI's `test vm ready HOST INSTANCE`, create's readiness phase,
  `test-vm HOST ready INSTANCE` and the retained RUN_DIR helper now share
  Rust readiness. It uses the run lock, recorded SSH target, ownership checks
  after waits, captured binaries, bounded probes and explicit profile boot
  expectations. It does not use embedded shell or `systemctl --wait`.
- Preserved signed-origin, base-unit, artifact bytes/metadata, SELinux,
  resolver/DNS, masked-unit, Homebrew and dotfiles acceptance checks. Both
  binaries are verified again after apply. Ready/deployed state is saved only
  after all checks and final domain ownership validation.
- Forty-nine VM tests pass, including success/retry, SSH/signed-boot timeout,
  bootstrap/apply failure, ownership loss, capture tampering, signed-origin
  refusal, guest security/user-environment failures and directory-based legacy
  identity lookup. Failure retains private diagnostic logs; ownership loss
  refuses further guest contact. The temporary boot policy lookup remains to
  be consolidated in workstream 2.
- Passed static-musl `cargo fmt --all --check`, offline workspace all-target
  Clippy with `-D warnings`, offline workspace tests/build, the routine Fedora
  base integration (both applies; no repeat service start/restart), readiness
  CLI help, ShellCheck for both changed helpers, and `git diff --check`.
- Live native/Flatpak readiness, rebase and retained-run import remain
  unverified. The user's VM and provider resources were not contacted or
  modified. No vault unlock or workstation installation was needed.

## Readiness retry recovery, 2026-10-03

- A readiness retry now persists `Started` before guest work, so failure after
  updating a previously `Ready` run cannot leave the prior success checkpoint
  in place. Only full acceptance plus a final owner check persists `Ready` and
  the captured deployed hashes.
- A guarded transport checks domain identity before and after every guest
  command and upload, including systemd service starts. A changed owner halts
  the sequence before subsequent service operations or diagnostics.
- Regression tests begin timeout/bootstrap/apply retry cases in `Ready` and
  verify the saved phase becomes `Started`; an ownership-loss case during the
  Brew start confirms dotfiles is not started afterward. Full validation is
  complete: formatting, all-target offline Clippy, workspace tests/build,
  Fedora repeat-apply integration, readiness CLI help, ShellCheck for both VM
  wrappers and `git diff --check` pass. Live VM acceptance remains pending.

## Retained VM update migration, 2026-10-03

- Rust now builds both binaries from Cargo-reported paths, checks recorded
  domain ownership, and delivers through the shared noninteractive SSH adapter.
  Updates and disposal use the same per-run lock. Original captured hashes are
  preserved; deployed hashes are saved only after both installed files verify.
- Adapter tests cover repeated delivery without upload/install, interrupted
  installation and retry, hash mismatch, cleanup-phase refusal, shared lock
  contention and rejection of nonregular manifest files. Forty VM tests pass.
- Passed `cargo fmt --all --check`, offline workspace all-target pedantic
  Clippy with `-D warnings`, offline workspace tests and static musl build.
  Passed `skillet test run beezelbot --phase base --image fedora:latest`:
  both applies succeeded and the repeat issued no service start/restart.
  ShellCheck for `bin/test-vm` and `git diff --check` passed.
- No live VM was contacted or changed: VM creation is running in the user's
  other panel. Live update, boot, and migration acceptance remain unverified.

## VM refactoring foundation, 2026-10-03

- Source slice: after roadmap commit `471068f`, added `skillet_vm` manifest
  types and shared CLI identity validation. No guest service behavior changed.
- Passed on the static-musl target, with the existing cross compiler on PATH:
  `cargo fmt --all --check`,
  `cargo clippy --offline --workspace --all-targets -- -D warnings`,
  `cargo test --offline --workspace`, and `cargo build --offline`, followed by
  `./target/x86_64-unknown-linux-musl/debug/skillet test run beezelbot --phase base --image fedora:latest`.
  The isolated Fedora container passed both applies with no repeat start/restart
  and was removed afterward. Eight new manifest tests cover strict import,
  preserved journals/hashes, metadata permissions and ownership/path refusal.
- Initial GNU-target checks under `/tmp` ran out of space. Only this task's
  temporary build directory was removed; subsequent builds used `/home`.
  The cross compiler was present outside the restricted filesystem view; no
  software was installed.
- Live creation/import/readiness/destruction checks are deferred. The user is
  creating a smoke VM in another panel; it and its artifacts were left alone.
  No vault, Cloudflare or Tailscale request was needed for this slice.

## Shared SSH transport contract, 2026-10-03

- Source slice: follows `67d8f54`. Added `GuestTransport` and `SshTransport`
  for literal executable/argument requests, explicit target and host-key policy,
  noninteractive SSH/SCP, and stdin delivery. No live SSH command was run.
- Subprocess I/O is memory-backed with concurrent pipe readers/writer, bounded
  output and complete-operation timeout. Tests cover a 1 MiB round trip, a
  blocked stdin writer, unconsumed input on nonzero exit, key/target validation,
  shell metacharacter quoting, upload traversal refusal, DNS/IPv6 targets and
  explicit host-key enrollment versus verification. Thirty-five VM tests pass.
- Passed static-musl workspace format, all-target pedantic Clippy,
  workspace tests/build and the routine Fedora base container integration.
  Readiness/delivery migration and live transport acceptance remain pending;
  no vault unlock, provider mutation or change to the user's VM was attempted.

## Interrupted local artifact removal, 2026-10-03

- Follow-up to `d799ace`: disposal now removes application artifacts before
  deleting authoritative/legacy recovery records. A deterministic local
  removal failure leaves both the versioned manifest and original identity/
  hash records readable; retry finishes artifact deletion.
- Twenty-seven VM tests, workspace tests, all-target pedantic Clippy,
  formatting, routine Fedora container integration and whitespace checks pass.
  This is local fault injection; live VM disposal remains deferred.

## Rust catalog and Cargo artifact selection, 2026-10-03

- Source slice: follows `d799ace`. Both list entry points delegate to Rust
  catalog/inspection, without rewriting manifests. Cargo JSON selection now
  supplies the container runner. Twenty-six VM tests cover catalog availability,
  partial-run discovery, non-default artifact paths/profiles, missing/duplicate
  artifacts, and existing ownership/cleanup cases.
- Passed static-musl formatting, workspace all-target pedantic Clippy,
  workspace tests/build, helper ShellCheck and whitespace checks. Also passed:
  `CARGO_TARGET_DIR=/home/giacomo/.cache/devmachine-target ./target/x86_64-unknown-linux-musl/debug/skillet test run beezelbot --phase base --image fedora:latest`.
  The runner selected the cache directory's Cargo-reported host binary rather
  than a conventional workspace path; both applies passed and cleanup completed.
- Live VM catalog/runtime checks and retained-run lifecycle acceptance remain
  deferred. No VM mutation, vault prompt or external provider call was attempted.

## Rust VM inspection and disposal, 2026-10-03

- Source slice: follows `544b8e5`. `test vm status` and both destroy entry
  points use `skillet_vm`. Native and Flatpak command contracts are covered
  without contacting libvirt. Destructive actions use the recorded UUID.
- Twenty-one VM regression tests pass, including absent guests, failed runtime
  queries, reused names/renamed UUIDs, wrong disks, literal subprocess arguments,
  bounded command timeout, concurrent disposal, external cleanup failure/retry,
  and local cleanup retry after the external phase was already completed.
- Passed the same static-musl workspace format, all-target pedantic Clippy,
  workspace tests/build and Fedora base integration commands listed above;
  also passed `shellcheck -x butane/bin/test-vm` and `git diff --check`.
- No live VM mutation, retained-run import, provider call or vault unlock was
  attempted. Live cleanup and Flatpak runtime acceptance remain unverified.
  The user's concurrent VM creation was left untouched.

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

## KeePass use-or-create smoke, 2026-09-28

- Fresh `clamps-test-credential-e2e` VM on port 2202 passed current Butane
  readiness, including signed uCore boot and the user environment. The updated
  Rust host command installed a generated dummy Pi-hole credential; full apply
  completed and Pi-hole became active. The guest credential is root-owned and
  mode 0600.
- `skillet test smoke clamps --port 2202 --identity
  ../butane/runs/clamps-test-credential-e2e/ssh/id_ed25519` passed all fixture
  cases and reboot persistence. Guest snapshots remain under
  `/var/lib/skillet-smoke/` on the retained VM.
- An empty disposable KDBX could not replace the VM's existing credential.
  After removing only that VM's dummy credential and Podman secret, the same
  vault created `skillet/hosts/clamps/pihole/web-password` and delivered it.
  A repeat delivery reopened the saved entry and completed full apply. The
  encrypted vault backup was retained, the host reported `present`, Pi-hole
  was active, and full apply reported `Result=success` and `ExecMainStatus=0`.
- On this workstation, both Skillet and `keyctl timeout` received permission
  denied when setting expiry on a new user key. Skillet unlinked that key and
  continued without caching the dummy unlock; the user keyring was empty after
  the run. The three-hour kernel expiry path remains unverified here. No
  production vault, production host, or physical disk was used.
- Follow-up: `keyctl timeout` succeeded for a dummy key stored in a named
  session keyring. Linking that keyring from the user keyring kept the same
  named ring and dummy key available to a later process. Skillet now uses that
  arrangement for the three-hour cache; the KeePass-backed end-to-end flow has
  not yet been repeated with this implementation.

## Post-delivery OS update/rebase and credential recovery, 2026-09-28

- On `clamps-test-credential-e2e`, the signed `latest` ref had a newer image
  available (digest `5d11f1003ffa1cb2a528c2e6943fa505dad86c2d3e5bf87b5330978111f3e0d2`).
  `rpm-ostree upgrade` staged it, and `test-vm clamps ready credential-e2e`
  passed after reboot. The encrypted Pi-hole credential remained present and
  decryptable; full apply returned success, the Podman secret was present, and
  Pi-hole was active.
- An explicit signed rebase to that same image digest also staged and booted.
  Readiness passed again; credential decryption and full apply succeeded, and
  Pi-hole returned active. A same-tag rebase had been a no-op because the refs
  were equal, so the newer signed image was staged first.
- To exercise delivery recovery, a runtime-only systemd drop-in temporarily
  forced `skillet-full-apply.service` to fail. Skillet `test vm provision
  clamps credential-e2e --rotate` returned the expected activation error after
  atomically installing a new encrypted credential. `systemd-creds decrypt`
  validated it; the existing Podman secret and running Pi-hole were retained.
  After removing the drop-in, normal `test vm provision clamps credential-e2e`
  reused the installed credential, full apply succeeded, the Podman secret ID
  changed, and Pi-hole remained active. The runtime drop-in was removed and
  systemd reloaded.
- This tests recovery after guest-side atomic credential installation when
  activation fails. A disconnect during the input stream or encryption is
  still untested. The software-only VM also reports that its systemd host key
  is not on encrypted media; production TPM binding remains open.

## Syncthing service, 2026-09-28

- On the named disposable `clamps-test-credential-e2e` VM, a full apply created
  `/var/lib/data/syncthing` as a Btrfs subvolume and started the separate
  `syncthing` Quadlet container on the shared clamps network. Container-name
  DNS resolved both bridge addresses, and `http://syncthing:8384` returned
  HTTP 200 from another container.
- The VM showed the Chef-equivalent host publications for GUI port 8384 and
  transfer port 22000 on IPv4 and IPv6. A repeated full apply preserved the
  container ID and configuration hash. Its empty data tree generated a VM-only
  device identity; no production Syncthing identity was copied or run.
- These are the previous session's runtime results. On the current workstation,
  `test-vm clamps list` reports this domain as `missing`, so no fresh VM
  rerun was made before committing. Real peer connectivity and production
  state transfer remain cutover work. The GUI publication must be removed or
  locally restricted before tailnet access can serve as its sole gate.

## Btrbk image and VM attempt, 2026-09-29

- Image workflow [run 36552978960](https://github.com/gbagnoli/ucore-images/actions/runs/36552978960)
  succeeded for `ucore-common` and all three host images. The published clamps
  image digest is
  `sha256:e6c0cd35641199ddb5e2d9ca0f281dc1d640126ae5b2a04fa87c65163cedfd7c`.
- The user created `clamps-test-smoke` with `./butane/bin/test-vm clamps
  create smoke`. The guest booted the expected signed image and confirmed
  `btrbk-0.32.7-1.fc44.noarch` is present, `btrbk.timer` is masked, and
  `/var/lib/data` is Btrfs subvolume `/data` on the expected UUID.
- The first readiness attempt hung during baseline convergence. Guest
  `systemd-sysctl.service` is `Type=oneshot`, `RemainAfterExit=yes`; passing
  `--wait` made systemctl wait for this persistent unit to stop. The rule was
  corrected: ordinary `systemctl start` and `restart` wait for the requested
  job. Skillet, credential delivery, the VM readiness helper, and its usage
  example now omit `--wait`. Rebuilt clamps completed base apply successfully.

## Btrbk snapshot verification, 2026-09-29

- Dummy-credential provisioning completed full apply. Pi-hole and Syncthing
  started, the `/var/lib/data/syncthing` Btrfs subvolume was present, and
  `/etc/btrbk/btrbk.conf` contained only that source with the Chef-equivalent
  hourly retention (`6h`, `24h 31d 6m`). `skillet-btrbk.timer` was active and
  enabled while the packaged daily `btrbk.timer` remained masked.
- `btrbk -n -v run` showed only `/var/lib/data/syncthing` as its source and
  `/var/lib/data/snapshots/syncthing/syncthing.<timestamp>` as its target.
  A real run created read-only snapshot `syncthing.20260929T1402`. A disposable
  sentinel was changed in the live tree, then restored from that snapshot into
  `/var/tmp/skillet-btrbk-restore`; byte comparison passed. The temporary
  restore subvolume and sentinel were deleted, while the btrbk snapshot was
  retained.
- Repeated full apply preserved the btrbk config and timer hashes and retained
  the existing snapshots. The timer fired at 14:00:30 and created
  `syncthing.20260929T1400`. After a VM reboot, Pi-hole, Syncthing, and
  `skillet-btrbk.timer` were active; the timer remained enabled, the package
  timer remained masked, both snapshots remained, and the next hourly run was
  scheduled for 15:00 CEST.
- `cargo run --release -p skillet -- test smoke clamps` passed the real-runtime
  systemd/Podman fixture checks through reboot. Local validation passed:
  formatting, 45 workspace tests, pedantic Clippy with warnings denied, the
  CI-style container command, `bash -n` and ShellCheck for `test-vm-ready`,
  and `git diff --check`. The CI-style command ran from `/tmp` because the
  checked-out workspace mount is read-only to shell commands.
- Fail-closed behavior for missing and wrong `/var/lib/data` mounts was already
  exercised against the shared storage configuration (see the shared Btrfs
  data storage section above); btrbk unit tests also reject a wrong mount
  before writing managed state. The disposable VM used an empty Syncthing
  subvolume. The user confirms Syncthing data is designed without nested
  subvolumes; the btrbk design treats selected sources as leaf subvolumes and
  does not support nested subvolumes.

## Generic private UI slice, 2026-10-01

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo
  build`, and the CI command `skillet test run beezelbot --phase base --image
  fedora:latest` passed. New tests cover a non-clamps Syncthing-only declaration
  in production and test, derived names, multi-service rendering, zone suffix
  validation, rejection of invalid/duplicate services, and receiver-side
  payload matching. A repeated Caddy apply test confirms unchanged generated
  config does not restart the proxy container. Existing container tests still
  pass.
- Fresh `clamps-test-generic-ui` completed Ignition, signed uCore rebase,
  base apply, Homebrew, and dotfiles installation using the pre-correction
  static host-binary unit. A `%H` experiment failed because systemd expands it
  to the full VM hostname, which is distinct from the host profile. The shared
  unit now invokes the generic CLI with `/etc/skillet/host`; clamps writes the
  stable profile `clamps`, and its credential-gated full-apply unit is in a
  clamps-specific include.
- On the existing `clamps-test-generic-ui` disposable VM, installed the current
  generic and clamps binaries, wrote the stable host profile, and used a
  temporary systemd drop-in to run the base unit through the generic
  `--host-file` invocation. The unit reported success while the VM hostname
  remained `clamps-test-generic-ui`. The real `skillet test smoke clamps
  --port 2202 --identity <run-dir>/ssh/id_ed25519` passed through reboot. This
  verifies the generic runtime path; a fresh VM from the updated Ignition and
  binary-staging flow is still pending. The prior guest evidence remains in its
  ignored run directory.
- Strict Butane compilation passed for the updated clamps config and both
  shared and clamps-specific unit fragments. Running the normal wrapper built
  and staged both current static binaries; decoded merged Ignition confirmed
  `/etc/skillet/host=clamps`, generic `skillet` commands for base/full/Caddy,
  and the host binary at `/var/usrlocal/bin/skillet-clamps`.
  `bash -n`, ShellCheck, and `git diff --check` also passed for the changed
  launchers. The exact CI integration command passed in a writable temporary
  checkout after the workspace mount rejected Cargo output; that run used the
  default CI test image and reported repeat apply with no extra service starts.
- The earlier Caddy gate check had no credentials and confirmed only that the
  unit remained skipped. No domain/token was delivered;
  Caddy startup, ACME issuance, HTTPS proxy access, renewal, and DNS lifecycle
  remain unverified. `beezelbot` has no Butane template, so its real-runtime
  service behavior remains unverified.

## Cloudflare UI lifecycle implementation, 2026-10-02

- `cargo fmt --all` passed.
- `CARGO_TARGET_DIR=/home/giacomo/.cache/devmachine-target
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
  cargo test --workspace --target x86_64-unknown-linux-gnu` passed (63 tests).
- `CARGO_TARGET_DIR=/home/giacomo/.cache/devmachine-target
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
  cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu --
  -D warnings` passed. Cloudflare tests use a local mock HTTP server and need
  no KeePassXC unlock, Cloudflare account, or live zone.
- The CI integration command `skillet test run beezelbot --phase base --image
  fedora:latest` passed using the GNU-target CLI. Its repeat apply issued no
  extra service starts.
- `skillet test vm list clamps` found `generic-ui` and `smoke` running. Updating
  `generic-ui` for the integration test was attempted, but `test-vm update`
  requires both x86_64 musl artifacts; the generic CLI artifact was missing.
  The musl Rust target is installed, and the existing cross compiler requires
  `/opt/x86_64-linux-musl-cross/bin` in PATH. The earlier diagnosis of a missing
  compiler was incorrect. No packages or toolchains were installed. The VM smoke script,
  `--with-ui` provisioning, real Cloudflare mutations, staging ACME, and HTTPS
  acceptance remain unverified.

### Musl build and live UI attempt, 2026-10-02

- With `/opt/x86_64-linux-musl-cross/bin` prepended to PATH and
  `CARGO_TARGET_DIR=/home/giacomo/.cache/devmachine-target`,
  `cargo build --release --target x86_64-unknown-linux-musl -p skillet
  -p skillet-clamps` passed using the existing toolchain.
- `skillet test vm update clamps generic-ui` installed both static binaries
  in the existing named disposable VM.
- `skillet test vm provision clamps generic-ui --with-ui` stopped at vault
  unlock: the kernel cache was unavailable and the tool process had no terminal
  (`reading database password from terminal: No such device or address`).
  No Cloudflare API or live certificate acceptance is claimed. User terminal
  unlock is needed before continuing retained-VM checks and the fresh
  destroy/create/provision/verify/dispose cycle.

### Cloudflare zone preflight permissions, 2026-10-02

- After user unlock, `skillet test vm provision clamps generic-ui --with-ui`
  reached Cloudflare and returned HTTP 403 / code 9109 during the configured
  zone lookup using the workstation issuer. This occurs before child-token
  issuance and DNS reconciliation. Caddy ACME/HTTPS remain unverified.
- The README had incorrectly excluded zone permissions from the issuer.
  Corrected setup requires Zone Read on configured zones as well as User API
  Tokens Read/Edit; DNS Write is still confined to child tokens. The rebuilt
  CLI reproduced the error with zone-preflight permission context.
- `cargo fmt --all --check`, workspace tests (64 tests), and all-target
  pedantic Clippy passed with `--target x86_64-unknown-linux-gnu` for tests and
  Clippy. The CI integration scenario `skillet test run beezelbot --phase base
  --image fedora:latest` passed; repeat apply issued no service restarts.
  `cargo build --release --target x86_64-unknown-linux-musl -p skillet` passed
  with the documented compiler PATH. The HTTP mock regression test verifies
  useful 403 context without token or configured zone ID disclosure.
- Live acceptance is pending issuer zone access being updated in Cloudflare;
  no live certificate issuance or fresh full cycle is claimed.

### Relative UI namespace, 2026-10-02

- `dns/cloudflare-zone-id` remains mandatory. Workstation production delivery,
  opt-in VM UI provisioning, and VM cleanup now resolve the optional
  `dns/ui-domain` as a relative prefix appended to the fetched zone domain.
  An absent entry defaults to `ui`; an empty entry is invalid. Full-domain
  values are also treated as relative and require deliberate local migration.
- `cargo fmt --all --check`, workspace tests (66 tests), and all-target
  pedantic Clippy passed; tests and Clippy used the GNU target. Tests cover
  default/multiple-label prefixes, environment isolation, always-appended
  zone suffixes, invalid prefixes and excessive total domain length.
- Both musl release binaries built successfully with the documented compiler
  PATH. `skillet test run beezelbot --phase base --image fedora:latest` passed
  with no repeat-apply service restart. The named `clamps-test-generic-ui` VM
  was updated with both binaries.
- `integration_tests/smoke-ssh.sh --target giacomo@127.0.0.1 --port 2202
  --identity ../butane/runs/clamps-test-generic-ui/ssh/id_ed25519
  --disposable-target --binary
  /home/giacomo/.cache/devmachine-target/x86_64-unknown-linux-musl/release/skillet
  --clamps-binary /var/usrlocal/bin/skillet-clamps` passed all fixture cases,
  failure recovery and reboot persistence. Evidence is retained in the guest
  under `/var/lib/skillet-smoke/`. This fixture check is not live ACME acceptance.
- Live Cloudflare/Caddy acceptance remains pending migration of the existing
  vault prefix. No new DNS mutations or certificate issuance are claimed for
  this change.

### Live issuer retry, 2026-10-02

- Retried `skillet test vm provision clamps generic-ui --with-ui` using the
  cached vault unlock. Zone lookup and relative namespace resolution succeeded.
- The first retry found that the saved issuer is account-owned and correctly
  has Account API Tokens Write. The original Skillet implementation called
  user-token endpoints, which returned HTTP 403 / code 9109. This endpoint
  mismatch was corrected by deriving the account ID from the selected zone and
  using account-token endpoints for discovery, creation, listing, and revoke.
- See the live account-token and Caddy result below; the earlier blocked state
  has been resolved.

### Account token, Cloudflare DNS, and Caddy live acceptance, 2026-10-02

- The account-owned issuer passed permission-group discovery and created the
  scoped child token. Cloudflare DNS reconciliation created the expected five
  owned records for the machine address, canonical UIs, and declared alias.
- Caddy's disposable container started and obtained Let's Encrypt staging
  certificates for Pi-hole, Syncthing, and the Syncthing alias using DNS-01.
  OpenSSL checks confirmed each certificate SAN matched the requested name and
  the issuer was staging.
- From the workstation on the tailnet, DNS resolved over IPv4 and IPv6. HTTPS
  returned Pi-hole `/admin/` login redirects (302) and Syncthing canonical and
  alias pages (200) over both address families. Bare Pi-hole `/` returns 403;
  its UI is at `/admin/`.
- `skillet test vm destroy clamps generic-ui` completed. It removed the owned
  Cloudflare records and child tokens through account endpoints, removed the
  Tailscale device, undefined the libvirt domain, and removed Cloudflare
  ownership metadata. The retained unrelated `clamps-test-smoke` VM remains.
- `cargo fmt --all --check`, workspace GNU tests (67 tests), all-target
  pedantic Clippy, and `skillet test run beezelbot --phase base --image
  fedora:latest` passed. The account create/list/revoke endpoints and Caddy
  config directory mount have regression coverage.
- This validates staging only. A request from outside the tailnet and
  production ACME issuance/renewal were not tested.

### Cloudflare API failure recovery tests, 2026-10-02

- Added local mocked-HTTP tests under `skillet/crates/cli/src/cloudflare_tests.rs`.
  They run as part of the existing workspace `cargo test` CI job and require
  no vault, Cloudflare account, credentials, or DNS zone.
- The token test models Cloudflare committing a named token while returning an
  error; retry lists and revokes that orphan before creating exactly one
  replacement. DNS reconciliation retries after a create failure and adopts
  the already committed records without duplicates. Cleanup retries after a
  partial delete failure and preserves a record owned by another marker.
- Focused Cloudflare tests pass (10 tests). `cargo fmt --all --check`, all 70
  workspace tests, `cargo clippy --all-targets -- -D warnings`, and
  `skillet test run beezelbot --phase base --image fedora:latest` all pass.

### Non-tailnet Caddy denial probe, 2026-10-02

- Caddy now returns a distinctive 403 body for requests outside the Tailscale
  IPv4/IPv6 ranges. Opt-in `test vm provision --with-ui` probes each declared
  canonical name and alias from the guest's loopback address, using the UI
  hostname for TLS SNI and the HTTP Host header.
- The user completed live `test vm provision clamps generic-ui --with-ui` on
  the disposable VM. Provisioning returned successfully, which means the
  loopback probe received the exact configured 403 for every declared UI name
  and alias. Follow-up inspection found the Caddy container running and three
  successful certificate issuance events in its logs.
- The Caddy renderer test, workspace suite, Clippy, and routine integration
  scenario pass. This loopback check does not replace a request from an
  independent network outside the VM. The disposable VM remains running for
  manual inspection.

### Pi-hole DNS behavior on smoke VM, 2026-10-02

- On the running `clamps-test-smoke` VM, Pi-hole was active and published DNS
  listeners on wildcard IPv4 and IPv6 addresses. Queries for a public test
  name succeeded over UDP and TCP through the VM's IPv4 and IPv6 interface
  addresses.
- A temporary Pi-hole v6 `dns.hosts` record using the documented
  `"IP HOSTNAME"` format resolved over both transports and address families.
  The original empty custom-host setting was restored and verified afterward.
  The unauthenticated admin page redirected to login (302), and the unauthenticated
  API auth endpoint returned 401.
- This VM uses libvirt's user-mode `passt` network with only SSH forwarded to
  the workstation. Its active firewalld zone has no explicit DNS service or
  port allowance. Guest-local queries do not establish client ingress, so LAN
  reachability and host-firewall behavior remain unverified; validate them on
  a bridged/physical clamps network before cutover. No production DNS records
  were supplied or changed.

### UniFi rupik source inventory, 2026-10-02

- The sanitized inventory log is `/tmp/rupik.log` on the workstation. Rupik is
  ARM64 and runs `jacobalberty/unifi:latest`, package `10.0.162-32076-1`, in a
  rootful host-network container as image user `unifi` (UID/GID 999). Podman
  reports host UID 999 as `systemd-coredump`; Chef separately creates host
  `unifi` UID/GID 2666. The data tree is owned by 999:999, mode 0750, and uses
  1.2 GiB. `/srv` is Btrfs; the image's `/unifi/run` is a separate generated
  Podman volume.
- Observed listeners: TCP 8080, 8443, 6789, 8843, 8880; UDP 3478, 10001. HTTPS
  on local 8443 returned 302. No UniFi nginx route appears in the Chef repo.
  Eight small monthly `.unf` backup files were listed through October 2026;
  validity and restore compatibility are unverified. Clamps is x86_64, so a
  backup restore from the ARM64 controller must pass on an isolated VM before
  any production migration. The full private controller settings were not
  included in this log.

### UniFi Skillet implementation, 2026-10-02

- Added reusable `skillet_unifi` support and enabled it from clamps full
  apply. It requires `/var/lib/data`, creates `/var/lib/data/unifi` as a Btrfs
  subvolume, mounts it at `/unifi`, and configures rootful Podman with mandatory
  host networking, the Chef image family, `User=unifi`, `TZ=Europe/Madrid`,
  restart-at-boot, and registry auto-update.
- Skillet updates only the subvolume root to numeric UID/GID 999; it does not
  recursively alter existing controller data and does not require matching
  host account names. The first disposable VM apply exposed that clamps uCore
  has no passwd entry for UID 999. Numeric ownership handling and regression
  tests were added in follow-up commit `6765ecb`.

### UniFi empty-controller VM acceptance, 2026-10-03

- Updated the running `clamps-test-unifi` disposable VM with the new `skillet`
  and `skillet-clamps` binaries using `cargo run --release -p skillet -- test
  vm update clamps unifi`.
- Retried `skillet-full-apply.service`; it completed with status 0. The
  `unifi.service` Quadlet is active and `podman inspect` reports
  `network=host` with `/var/lib/data/unifi:/unifi` mounted.
- `/var/lib/data/unifi` is a Btrfs subvolume with owner `999:999` and mode
  `750`. The container started and emitted UniFi application initialization
  logs.
- Repeated the full apply. It completed with status 0, UniFi remained active,
  and the container ID stayed unchanged. Backup restore, reboot persistence,
  and expected host listener checks remain pending; see
  [the UniFi plan](../plan/CLAMPS-UNIFI.md).

### Rust VM local artifact preparation, 2026-10-03

- `skillet_vm::provisioning` now owns preparation of the disposable VM's copied
  disk and Ed25519 SSH key. Disk copies are staged beside the destination and
  atomically renamed; a SHA-256 sidecar records the selected image. Repeated
  preparation reuses a byte-identical disk and matching key pair, repairs a
  changed image while the run is still `Preparing`, and rejects partial keys,
  symlinked inputs, or progressed runs. The hidden `skillet test vm
  prepare-local` command bridges `coreos-install` to this library and returns
  the public key for Ignition rendering.
- `cargo fmt --all --check`, workspace pedantic Clippy with warnings denied,
  all 131 workspace tests, and `cargo build --offline --workspace` passed.
  `bash -n` and ShellCheck passed for `coreos-install`; `git diff --check`
  passed. The CI-style `cargo run --release -p skillet -- test run beezelbot
  --phase base --image fedora:latest` passed, including repeat apply without
  service restarts.
- No VM was created or changed for this slice. Template staging, Butane
  compilation and native define/start remain in the Bash installer; Flatpak
  creation and end-to-end VM acceptance remain pending.

### Rust VM Butane source staging, 2026-10-03

- `skillet_vm::staging` now copies the selected host config, `.bu` includes,
  and both captured binaries into the owned run directory. It specializes the
  guest hostname, injects the validated disposable SSH key, removes image
  binary entries and VM-only password-expiry resources, and adds the test-only
  sudoers file. Artifact hashes and `run.conf` compatibility metadata are
  written atomically. Repeated staging is idempotent and requires the current
  persisted run to remain `Preparing` while holding its run lock.
- `coreos-install` delegates disk/key preparation and staging to hidden Rust
  VM commands. Bash now invokes the standalone Butane compiler on the Rust-
  staged tree. Native define/start and tool-version capture remain in Bash.
- Formatting, workspace Clippy with warnings denied, all 134 workspace tests,
  workspace build, `bash -n`, ShellCheck, and `git diff --check` passed. The
  CI-style beezelbot base container scenario passed, including repeat apply
  without service starts.
- No VM was created or changed. The staged tree was exercised through fixture
  tests, but a fresh VM create through the combined flow was not verified.
  Native define/start remain in Bash; Flatpak creation is still unverified.

### Rust native VM define and start, 2026-10-03

- Native VM creation now resolves the x86_64 HVM emulator from the selected
  libvirt connection, captures tool versions, renders the owned domain XML,
  defines and starts the domain, and records its UUID in Rust. The lifecycle
  validates UUID and disk ownership after each backend observation. A retry
  after define or start can continue from the manifest and current domain
  state. `test-vm` delegates native resume to the same Rust command; Flatpak
  `virt-install` remains on its compatibility path.
- Fake-backend coverage verifies successful creation, recovery from an
  existing defined domain, idempotent running-domain retries, refusal of a
  foreign UUID, start-failure retry, and x86_64 emulator selection. The VM crate's 62 tests and the
  full workspace test suite and pedantic Clippy with warnings denied passed.
  Formatting, ShellCheck, Bash syntax, and `git diff --check` passed.
- No live VM was created or changed. The combined native flow and Flatpak
  creation remain to be exercised on their respective backends.

### Rust VM reboot and guest access, 2026-10-03

- `test-vm` now delegates reboot, interactive SSH, and guest diagnostics to the
  Skillet CLI. Rust holds the per-run lock, validates domain UUID/disks before
  guest access, and validates ownership after each operation. Reboot marks a
  previously Ready run Started before issuing the backend command, so stale
  readiness cannot survive an interrupted reboot. Interactive SSH attaches a
  terminal directly; captured logs remain bounded through the shared transport.
- Unit coverage verifies that interactive SSH allocates a terminal and uses
  the manifest target, and that a reboot invalidates Ready state. Workspace
  tests and pedantic Clippy passed; shell checks and formatting passed.
- No live VM was contacted. Reboot and interactive access remain unverified
  against a live guest in this slice. The CI-style container integration
  command was unavailable: Podman cannot set the sticky bit on
  `/run/user/4000/libpod` because the filesystem is read-only. No container
  was started.

### Rust Flatpak VM creation adapter, 2026-10-03

- Added a bounded `virt-install` adapter that uses the recorded Flatpak runtime
  and preserves the manifest UUID, disk, Ignition, QEMU user-networking, and
  forwarded SSH port. Flatpak definition now uses the common Rust phase and
  ownership recovery logic, including a retry when `virt-install` created or
  started the guest before an interrupted response. `coreos-install` delegates
  both native and Flatpak domain creation; Bash no longer records the domain
  UUID or start phase.
- Fake adapter and lifecycle coverage checks the executable wrapper, runtime
  environment, UUID, disk/network arguments, successful transition and retry
  behavior. The workspace suite passed, including 66 VM crate tests, and
  pedantic Clippy passed; formatting, ShellCheck, Bash syntax, and
  `git diff --check` passed.
- No Flatpak VM was created. Live acceptance remains unverified on Bazzite.

### Rust-owned VM create orchestration, 2026-10-03

- `skillet test vm create HOST INSTANCE` now owns the full create path: backend
  selection/runtime startup, image resolution, guest binary build, disk/key
  preparation, Butane staging/compilation, native or Flatpak define/start, and
  readiness. Create holds a dedicated per-run orchestration lock while calling
  the lower-level locked operations. Repeated create skips the SSH-port
  availability probe once a guest is already running. It preflights KVM and
  the forwarding port before expensive preparation. `bin/test-vm` is now a
  narrow compatibility argument adapter; the separate Bash `coreos-install`
  and hidden preparation/staging/creation CLI commands were removed.
- Passed `cargo fmt --all --check`, offline workspace tests (72 VM crate tests
  plus all other workspace suites), and offline workspace all-target Clippy
  with warnings denied. Built the static-musl CLI using
  `PATH=/opt/x86_64-linux-musl-cross/bin:$PATH` and the configured external
  Cargo target directory. `shellcheck -x` and `bash -n` passed for all affected
  VM/compiler wrapper scripts. CLI create help and `git diff --check` passed.
- Passed the CI integration command:
  ```bash
  CARGO_TARGET_DIR=/home/giacomo/.cache/devmachine-target \
    /home/giacomo/.cache/devmachine-target/x86_64-unknown-linux-musl/debug/skillet \
    test run beezelbot --phase base --image fedora:latest
  ```
  Both applies
  succeeded and the repeat issued no service restart; the test container was
  removed.
- Live VM acceptance was not run because this workstation has no `/dev/kvm`
  device (`stat /dev/kvm` reports “No such file or directory”). The recorded
  `clamps/smoke` run was reported as unavailable by `test vm list`; it was not
  altered. Native/Flatpak create, readiness, reboot, and destroy remain
  unverified live.

### Canonical host profiles, 2026-10-03

- Moved host service composition and boot expectations into the canonical
  `skillet_hosts` crate. Profile-level unit coverage checks all current hosts'
  service sets, UI ports/aliases, credential consumers, network parameters,
  mount needs, and snapshot configuration. Added a profile with a credentialed
  Pi-hole service without UI exposure, and an explicit no-service baseline.
  Unknown hosts are rejected before effects. The smoke fixture is now selected
  by a separate hidden test command rather than a synthetic host identity.
- Passed `cargo fmt --manifest-path ublue/skillet/Cargo.toml --all`,
  `cargo test --offline --manifest-path ublue/skillet/Cargo.toml --workspace`,
  and `cargo clippy --offline --manifest-path ublue/skillet/Cargo.toml
  --workspace --all-targets -- -D warnings`. Passed ShellCheck and Bash syntax
  checks for `smoke-ssh.sh` and `smoke-guest.sh`.
- Passed the CI container command using the Cargo-reported beezelbot binary:
  `ublue/skillet/target/x86_64-unknown-linux-musl/debug/skillet test run
  beezelbot --phase base --image fedora:latest`. Both applies succeeded and
  the test container was removed.
- Live base/full apply on a retained VM remains unverified because `/dev/kvm`
  is absent. No VM was created, contacted, or changed in this slice.

### Injected account, credential, and ownership inputs, 2026-10-04

- Host composition now receives systemd credential values explicitly. The CLI
  loads only the profile-declared credentials needed by the selected phase.
  Account lookups use the injected system boundary. Podman reads subordinate
  ID data via the injected file resource and refuses missing, malformed,
  duplicate, overflowing, or undersized ranges instead of using a fixed range.
- Directory ownership now uses one value supporting account names or numeric
  IDs. The numeric-ID path does not require a host account, and the fake keeps
  mode/ownership state through later existence-only checks. Tests preserve a
  child file across repeat apply and cover file-versus-directory errors,
  injected filesystem failures, account-ID mismatch, and mapping failures.
- Passed offline workspace tests for core, Podman, UniFi, and full workspace;
  pedantic Clippy with warnings denied; formatting; ShellCheck and Bash syntax
  checks; and the CI container integration command (beezelbot base apply twice,
  then remove the container).
- Workstream 3 remains in progress: the remaining broad file/system trait
  decomposition and live ownership/mount checks are outstanding. No live VM
  was changed; `/dev/kvm` is absent on this workstation.

### Cohesive file effect capabilities, 2026-10-04

- Split file access into read, mutation, and Btrfs storage traits. Recorder,
  local adapter, and fake implement those contracts individually. Updated
  service and Podman recipe bounds to declare the operations their composition
  uses; retained the aggregate for host composition during migration.
- Passed `cargo fmt --manifest-path ublue/skillet/Cargo.toml --all`, offline
  workspace tests, and offline workspace all-target Clippy with warnings
  denied. Passed `git diff --check`.
- Passed the CI container integration command:
  `ublue/skillet/target/debug/skillet test run beezelbot --phase base --image
  fedora:latest`. Both applies succeeded, the second caused no service restart,
  and the test container was removed.
- System capability splitting and live VM ownership/mount acceptance remain
  pending. This workstation has no `/dev/kvm`; no VM was changed.

### Cohesive system effect capabilities, 2026-10-04

- Split system effects into account lookup, account mutation, Podman-secret,
  and service capabilities. Narrowed Podman, hardening, service, and backup
  crate bounds; host composition keeps the aggregate trait. Linux, recorder,
  and mock adapters implement each capability.
- Passed workspace formatting, offline workspace tests, and offline workspace
  all-target Clippy with warnings denied. The full CI command
  `ublue/skillet/target/debug/skillet test run beezelbot --phase base --image
  fedora:latest` exited 0: both applies succeeded, repeat apply issued no
  restart, and the disposable test container was removed.
- Live ownership and Btrfs mount acceptance remains unverified because this
  workstation has no `/dev/kvm`; no VM was changed.

### Shared guest credential delivery, 2026-10-04

- Production host secret delivery and disposable VM provisioning now call the
  same `skillet_vm::credential::install` API over `GuestTransport`. The helper
  validates host/credential/unit inputs, supports immediate or deferred unit
  activation, delivers bytes on stdin, and returns sanitized guest errors.
- Tests cover stdin-only delivery, deferred activation arguments, invalid
  inputs before transport, and remote failure diagnostics. Passed offline
  workspace tests, workspace formatting, and strict all-target Clippy.
- The CI container integration command exited 0. Both base applies succeeded,
  repeat apply did not restart a service, and the disposable test container
  was removed. VM credential rotation/recovery acceptance remains pending; no
  `/dev/kvm` is available here.

### Tailscale Rust HTTP client, 2026-10-04

- Replaced the curl-based Tailscale API adapter with the workspace `reqwest`
  blocking client. The endpoint is injectable through a test-only constructor;
  production uses the HTTPS endpoint with bounded connect/request timeouts and
  Rustls.
- Local HTTP fixture tests cover OAuth auth-key creation, bearer auth, tagged
  device lookup/deletion, and a forbidden response. Full workspace
  formatting/tests/strict Clippy passed. The static-musl CLI built with
  `PATH=/opt/x86_64-linux-musl-cross/bin:$PATH`.
- CI container integration exited 0: both applies succeeded, repeat apply
  issued no service restart, and the container was removed. No live Tailscale
  API request was made.

### Workstation vault library, 2026-10-04

- Added `skillet_workstation` and moved KeePassXC opening, exact secret lookup,
  atomic encrypted save, backup/conflict checks, XDG location, lock operation,
  and named-session kernel cache out of CLI code. The API returns `thiserror`
  errors and keeps the unlocked database/password private.
- Workstation tests passed for exact entries, XDG fallback, correct/wrong
  KDBX passwords, symlink-target replacement and backup, concurrent-change
  refusal, absent-vault error before prompting, and three-hour cache expiry and
  clearing.
- Passed workspace formatting, tests, strict all-target Clippy, and static
  musl build. CI container integration exited 0; both applies succeeded,
  repeat apply did not restart, and the container was removed. Live credential
  delivery and rotation recovery on a disposable VM remain unverified because
  `/dev/kvm` is unavailable.

### Cloudflare workstation provider library, 2026-10-04

- Moved the Cloudflare SDK adapter, scoped account-token issuance/revocation,
  DNS reconciliation, desired record validation, and the local HTTP fixtures
  into `skillet_workstation::cloudflare`. The library uses a `thiserror`
  boundary preserving SDK/JSON sources; the CLI calls the library API.
- The 10 Cloudflare provider tests passed, including DNS reconciliation and
  ambiguous-token retry cases. Full workspace formatting/tests/strict
  all-target Clippy and the static-musl build passed. CI container integration
  exited 0; both applies succeeded, repeat apply issued no restart, and its
  disposable container was removed. No live Cloudflare API mutation was made.

### Tailscale workstation provider library, 2026-10-04

- Moved OAuth, auth-key, tagged-device lookup, and device-removal operations
  into `skillet_workstation::tailscale`; removed the CLI's direct `reqwest`
  dependency. Provider errors use `thiserror` and preserve transport sources.
- Local HTTP fixture tests passed for token/auth-key requests, tagged lookup and
  removal, and API errors. Workspace formatting, offline tests, strict
  all-target Clippy, and the CI container integration command passed. The
  integration command applied the beezelbot base twice, confirmed the repeat
  caused no service restart, and removed its disposable container.
- Live Tailscale API/VM acceptance was not run. Lifecycle ownership and
  production-versus-test orchestration remain pending in workstream 4.

### Explicit workstation environment policy, 2026-10-04

- Added typed production/test provisioning policy for UI domain lookup,
  shared Cloudflare zone lookup, live versus staging ACME, Cloudflare token
  lifetimes, cleanup-token lifetime, and production/disposable Tailscale tags.
  Production UI delivery and disposable VM issue/cleanup now use that policy.
- The two policy unit tests and full offline workspace tests passed. Strict
  workspace all-target Clippy passed. The CI container integration exited 0:
  both applies succeeded, the second caused no service restart, and the
  disposable container was removed. No provider or VM resources were changed.

### Podman host-wide DNS policy, 2026-10-04

- Moved Aardvark's host-wide DNS listener config out of each container apply.
  The canonical profile selects the policy when Pi-hole is declared; host
  composition writes it before service recipes. Container apply now only owns
  its Quadlet and declared network.
- Podman and host-profile tests passed, including idempotent listener config
  and profile selection. Full offline workspace tests, strict all-target
  Clippy, formatting, and the CI container integration passed. The CI
  beezelbot base fixture applied twice without restart and its container was
  removed.
- Live Pi-hole port 53 and service-name DNS behavior was not verified; no
  `/dev/kvm` is available on this workstation.

### Explicit Podman process identity, 2026-10-04

- Replaced the ambiguous container UID/GID + host-user fields with explicit
  image-default, named, and numeric identities. Namespace mapping is a separate
  opt-in input requiring an existing host account and subordinate ranges; the
  generic adapter no longer creates accounts. UniFi's named process user now
  comes from the typed identity field.
- Podman and UniFi tests passed for image-default, named, numeric, mapped
  identity, map-range validation, and rejection of raw typed-field conflicts
  before effects. Full offline workspace tests, strict all-target Clippy,
  formatting, and the CI container integration passed. The container applied
  beezelbot base twice with no restart on the second apply, then was removed.
- No live container identity or ownership checks ran; `/dev/kvm` is unavailable
  on this workstation.

### Typed Podman networks and port publications, 2026-10-04

- Host composition now creates each profile bridge network once; application
  recipes attach by network name. Container network mode and port bindings are
  typed values. Dual-stack Pi-hole, Syncthing, and Caddy publications plus
  UniFi/Tailscale host networking retain their prior intent. Raw directives
  that conflict with typed fields fail validation.
- Podman, service, and full workspace tests passed. Strict all-target Clippy
  passed. The CI container integration exited 0; beezelbot base applied twice
  without a restart on the repeat and its container was removed.
- No named-VM DNS/discovery or service networking test ran; `/dev/kvm` is not
  available here.

### Shared service-data mount dependency, 2026-10-04

- Added a reusable typed Podman mount dependency that renders preparation,
  bind, ordering, and mountpoint assertion directives. Persistent Pi-hole,
  Syncthing, UniFi, Caddy, and Tailscale containers use it. Ignition still owns
  filesystem and graphroot preparation; applications own their child data
  directories and ownership.
- The dependency rendering/rejection tests and full workspace tests passed.
  Strict all-target Clippy and the CI container integration passed; both
  beezelbot base applies succeeded with no restart on the second apply and the
  disposable container was removed.
- Live mount/boot-order checks need a named VM and remain unverified because
  `/dev/kvm` is unavailable here.

### Shared activation, sanitized recording, and test fixture split, 2026-10-04

- Added shared activation/revision handling to hardening, Podman containers,
  and the btrbk timer. Unit tests verify changed definitions, stopped services,
  no-ops, oneshot semantics, and recovery after failed reload/start/restart.
- `apply --record` now writes versioned YAML atomically after both successful
  and failed applies. Records include operation outcomes, omit content and
  secret fingerprints, and do not retain raw effect error messages. Tests cover
  failed applies and combined apply/record persistence failure.
- Extracted `skillet-smoke-fixture` from production host/CLI dispatch and moved
  its guest entrypoint to a standalone shell file. Cargo-reported artifact
  selection follows the invoked Skillet target/profile. The smoke runner
  verifies the fixture transfer and the installed generic/host binaries against
  the expected recorded hashes. CI includes Butane and helper paths, all-target
  formatting/Clippy/tests, extensionless shell helpers, and the Fedora
  container integration command.
- Local full workspace tests, strict all-target Clippy, formatting, ShellCheck,
  and the CI container integration passed. The beezelbot base phase applied
  twice; the repeat issued no service start/restart and the disposable container
  was removed. The container runtime needed SIGKILL after its 10-second stop
  timeout during cleanup; this did not fail the test.
- Fresh VM activation/reboot, host-profile acceptance, and retained-run recovery
  are unverified because this workstation has no `/dev/kvm`.

### Typed Podman volume-root ownership, 2026-10-04

- Added optional typed host mode and ownership to each bind-volume declaration.
  Podman applies these only to the mount root. UniFi now declares mode `0750`
  and numeric UID/GID `999` as part of its volume; other existing volumes leave
  ownership unspecified. Descendant data is not recursively changed.
- The focused Podman and UniFi tests passed. The full offline workspace test
  suite, strict all-target Clippy, formatting, and the documented beezelbot
  Fedora container integration passed. The integration applied twice; the
  second apply did not restart services. Podman needed SIGKILL after its
  10-second stop timeout while removing the test container.
- A regression test confirms the typed metadata does not change the existing
  Quadlet bytes. VM verification of actual volume ownership, descendants, and
  service restart behavior remains unverified because `/dev/kvm` is absent.

### Workstation-owned disposable provisioning state, 2026-10-04

- Moved Cloudflare ownership JSON, Tailscale pending-enrollment markers, and
  enrolled-device records from the CLI into
  `skillet_workstation::provisioning_state`. Filenames are unchanged. New
  journals include separate host/environment/instance identity fields; readers
  accept the prior JSON and hostname-only marker formats with exact identity
  checks. Writes are atomic and mode `0600`; reads and writes reject
  symlinks/non-regular files. Retrying with another VM identity fails closed.
- Workspace all-target tests, strict Clippy, and formatting passed. State tests
  cover private file mode, state round trips, symlink refusal, idempotent marker
  writes, typed identity conflict refusal, legacy state compatibility, and
  marker cleanup. The documented
  beezelbot Fedora container integration passed twice without a restart on the
  second apply; cleanup escalated to SIGKILL after its 10-second timeout.
- No provider resources or live VMs were changed. Full enrollment/cleanup
  acceptance remains pending; `/dev/kvm` is absent.

### Shared private UI provisioning plan, 2026-10-04

- Added `skillet_workstation::ui_provisioning` as the shared derivation for
  profile-declared Caddy sites and Cloudflare A/AAAA/CNAME records. Production
  and disposable VM delivery now use the same plan and explicit environment
  policy. It reads the services from the canonical host profile, including
  rendered aliases, without host-specific UI code in the delivery path.
- The three focused UI plan tests passed, followed by all workspace tests
  (including 33 workstation tests), strict all-target Clippy, and format check.
- `cargo run --offline --manifest-path skillet/Cargo.toml --bin skillet --
  test run beezelbot --phase base --image fedora:latest` exited 0. Both applies
  succeeded; the repeat issued no service start/restart. Podman needed SIGKILL
  after its 10-second stop timeout during test-container cleanup.
- This exercised derivation and the existing base container fixture only. No
  Cloudflare mutation, credential delivery, or live VM check was performed.
  `/dev/kvm` is unavailable on this workstation.

### Tailscale guest status parsing boundary, 2026-10-04

- Moved guest `tailscale status --json` interpretation from CLI orchestration
  into `skillet_workstation::tailscale`. The provider returns no addresses
  until its backend is running and rejects malformed or incomplete running
  responses; the CLI retains only remote invocation and bounded retry policy.
- All seven Tailscale provider tests passed, including running/non-running,
  invalid-address filtering, malformed JSON, and missing address arrays. The
  full workspace tests, strict all-target Clippy, and formatter check passed.
- The documented beezelbot container integration built its target executable
  but produced no runtime output for over 45 seconds. No matching Skillet or
  Podman worker appeared in the process listing; the command was interrupted.
  Treat this integration as unverified for this slice.
- No VM or live provider API was contacted. `/dev/kvm` remains unavailable.

### Shared Tailscale guest enrollment retry, 2026-10-04

- Moved the bounded retry loop into `skillet_workstation::tailscale` while
  keeping the guest SSH probe in the CLI. Empty status and transient probe
  errors retry until the supplied timeout; tests cover delayed success and
  timeout without sleeping.
- All 9 Tailscale provider tests passed. Workspace format, all-target tests,
  and strict all-target Clippy passed.
- The beezelbot Fedora base integration exited 0: both applies succeeded and
  the repeated apply issued no service start/restart. The disposable container
  needed SIGKILL after Podman's 10-second stop timeout during cleanup. This
  successful retry supersedes the stalled integration attempt recorded above.
- No VM or provider API was contacted. `/dev/kvm` remains unavailable.

### Fail-closed Tailscale enrollment probe, 2026-10-04

- Initial enrollment now propagates SSH/status command errors rather than
  interpreting them as an empty device state. The new regression test confirms
  a failed guest status command refuses enrollment even when stdout resembles
  a valid `NeedsLogin` response.
- The focused regression test, full offline workspace tests, strict all-target
  Clippy, and format check passed. The beezelbot Fedora base integration exited
  0; both applies succeeded, and the repeat issued no service start/restart.
  Podman needed SIGKILL after its 10-second stop timeout while cleaning up the
  disposable container.
- No live VM or provider API was used. `/dev/kvm` remains unavailable.

### Shared deferred credential-set delivery, 2026-10-04

- Added `skillet_vm::credential::install_set`, which validates the full
  duplicate-free credential set before any guest mutation and applies a single
  consumer/activation policy. Production and disposable Caddy flows install
  `caddy_sites` and `cloudflare_acme_token` as one deferred set, then activate
  Caddy after successful delivery.
- Credential delivery tests passed for stdin-only payloads, deferred
  activation, complete-set validation, duplicate refusal, and guest failure.
  Workspace formatting, all-target tests, and strict all-target Clippy passed.
- The beezelbot Fedora base integration exited 0: two applies succeeded, the
  repeat issued no service start/restart, and the disposable container was
  removed. Podman needed SIGKILL after its 10-second stop timeout.
- No VM credential delivery or live provider API was run; `/dev/kvm` is
  unavailable.

### Workstation-owned disposable Tailscale enrollment, 2026-10-04

- Moved the test VM enrollment transaction into
  `skillet_workstation::tailscale_enrollment`. The CLI supplies typed host,
  environment, instance, VM hostname, run directory, provider credentials, and
  verified guest transport. The workflow persists intent, reuses an existing
  enrollment, refuses failed status probes before auth-key creation, delivers a
  one-use key only after a successful empty observation, verifies the tagged
  device, and saves the ownership record before removing pending state.
- Four orchestration tests passed with fake guest transport and provider:
  existing enrollment, new enrollment/key delivery, failed-probe recovery
  state, and refusal of production policy. Full workspace tests, strict
  all-target Clippy, and formatting passed.
- The beezelbot Fedora base integration exited 0: both applies succeeded, the
  repeat issued no service start/restart, and the disposable container was
  removed. Podman needed SIGKILL after its 10-second stop timeout.
- No VM or live Tailscale API was contacted. `/dev/kvm` is unavailable.

### Workstream 3 ownership API audit, 2026-10-04

- Static search found no `/etc/subuid` or `/etc/subgid` reads outside the
  Podman adapter and no recipe reads of `CREDENTIALS_DIRECTORY`; account/NSS
  lookups remain in the system adapter. File, systemd, and Btrfs observations
  remain inside their declared core adapters.
- The ownership contract still has a concrete gap: file mutation accepts
  separate name-only owner/group arguments while directory mutation accepts
  the shared named-or-numeric `Ownership` type. A proposed broad core API
  migration was rejected by automatic review because it affects filesystem
  behavior across recipes. No source changes were made for that rejected
  migration. `/dev/kvm` is absent, so privileged ownership and mount checks
  remain unavailable.

### Workstation-owned non-UI credential delivery, 2026-10-04

- Moved production Pi-hole password reuse/use-or-create and Tailscale auth-key
  creation/delivery into `skillet_workstation::credential_delivery`. Disposable
  Pi-hole secret reuse/rotation now uses the same module. The shared
  `vault::SecretStore` and `VaultSecretStore` adapter also back Caddy token
  provisioning. The CLI now selects its typed workflow and constructs the
  verified transport.
- The workstation suite passed (66 tests), including missing/existing Pi-hole
  secrets, refusal when the guest already has an untracked secret, disposable
  reuse/rotation and inspection failure, and production-tagged Tailscale key
  delivery. Full workspace tests, strict all-target Clippy, and formatting
  passed.
- The documented Fedora Podman base integration exited 0: both applies
  succeeded; the repeat issued no service start/restart and the container was
  removed. Podman needed SIGKILL after its 10-second stop timeout.
- No production vault, live provider API, or named VM was used. Production
  credential acceptance remains unverified; `/dev/kvm` is unavailable.

### Workstation-owned persistent private UI provisioning, 2026-10-04

- Moved persistent Caddy provisioning into one workstation operation. It checks
  the host profile, resolves the tagged Tailscale device, builds the shared UI
  plan, reuses or mints a zone-scoped ACME token, migrates the prior production
  KeePass entry when present, reconciles owned DNS, installs both Caddy
  credentials before activation, and uses the existing verified guest
  transport. Failed encrypted vault persistence revokes the newly issued
  token. The CLI now supplies inputs and dispatches the operation.
- Fake store/provider/guest tests cover token reuse, issuance-before-DNS,
  legacy migration, vault conflict refusal, failed-save revocation, and
  deferred activation after the full credential set. Full workspace tests,
  strict all-target Clippy, and formatting passed (56 workstation tests).
- The beezelbot Fedora base integration exited 0: both applies succeeded; the
  repeat issued no service start/restart and the container was removed. Podman
  needed SIGKILL after its 10-second stop timeout during cleanup.
- No vault or live provider credentials were used here. Caddy certificate
  acceptance and Tailscale device lookup on a named production host remain
  unverified; `/dev/kvm` is unavailable.

### Workstation-owned disposable Caddy guest delivery, 2026-10-04

- Moved disposable Caddy credential-set installation, activation, per-name
  non-tailnet denial probes, and superseded-token revocation into
  `skillet_workstation::ui_provisioning`. Retry timing is bounded per hostname.
  Old tokens are retained until guest activation and every canonical UI/alias
  probe succeeds.
- Fake guest/provider tests cover deferred credential delivery, all-name probe
  arguments, revocation after success, and retaining older credentials when
  the denial response fails. Full workspace tests, strict all-target Clippy,
  and formatting passed (58 workstation tests).
- The beezelbot Fedora base integration exited 0: both applies succeeded; the
  repeat issued no service start/restart and the container was removed. Podman
  needed SIGKILL after its 10-second stop timeout during cleanup.
- No VM, vault, or live provider API was used. Live Caddy ACME/HTTPS acceptance
  remains pending because `/dev/kvm` is unavailable.

### Workstation-owned disposable Cloudflare UI transaction, 2026-10-04

- Moved disposable UI token issuance and DNS reconciliation into
  `skillet_workstation::ui_provisioning`. It persists intent before token
  creation, checks the vault snapshot immediately before issuing, saves token
  ID/expiry before DNS changes, and saves owned record IDs after reconciliation.
- Six UI provisioning tests passed, including write-ahead ordering, vault
  change refusal, and retaining token ownership when DNS reconciliation fails.
- The full workspace tests, strict all-target Clippy, and formatting checks
  passed (44 workstation tests in the workspace run).
- The documented beezelbot Fedora base integration exited 0: both applies
  succeeded; the repeat issued no service start/restart. Podman needed SIGKILL
  after its 10-second stop timeout during cleanup.
- Guest credential delivery, Caddy HTTPS acceptance, and live Cloudflare API
  behavior were not exercised here. `/dev/kvm` is unavailable.

### Workstation-owned disposable Cloudflare cleanup, 2026-10-04

- Moved disposable UI cleanup into `skillet_workstation::ui_provisioning`.
  Before provider mutations, it checks the test policy, typed VM identity,
  deterministic token/marker names, configured zone, and current relative UI
  namespace. It removes marker-owned DNS, revokes disposable and cleanup
  tokens, then removes the journal. Failures leave ownership state retryable;
  malformed and symlinked state paths are refused.
- Cleanup tests cover successful revocation and journal removal, repeated
  cleanup with no journal, identity mismatch before mutation, and DNS failure
  retaining the journal. State tests cover regular-file presence/removal and
  symlink refusal. The full workspace tests, strict all-target Clippy, and
  formatting checks passed (48 workstation tests).
- The beezelbot Fedora base integration exited 0: both applies succeeded; the
  repeat issued no service start/restart and the container was removed. Podman
  needed SIGKILL after its 10-second stop timeout during cleanup.
- No VM or live Cloudflare API was contacted. `/dev/kvm` is unavailable.

### Workstation-owned disposable Tailscale cleanup, 2026-10-04

- Moved test-device cleanup into `skillet_workstation::tailscale_enrollment`.
  The operation validates the disposable environment and typed host/instance/
  VM hostname, verifies ownership journals, asks the provider to remove only
  the recorded tagged device, and removes recovery files after provider success
  or an already-absent device response. Provider errors retain the journal.
- The workstation crate tests passed (51 tests), including provider failure
  retry, recorded-device identity verification, local journal cleanup, and
  identity mismatch refusal before provider calls. Full workspace tests,
  strict all-target Clippy, and formatting also passed.
- The beezelbot Fedora base integration exited 0: both applies succeeded; the
  repeat issued no service start/restart and the container was removed. Podman
  needed SIGKILL after its 10-second stop timeout during cleanup.
- No VM or live Tailscale API was contacted. `/dev/kvm` is unavailable.

### Manifest-backed disposable provisioning transport, 2026-10-04

- Removed the disposable provisioner's call through the `test-vm` compatibility
  wrapper and its parser for legacy `run.conf`. It now loads the typed run
  manifest, holds the per-run lock, uses its recorded SSH target, and validates
  the expected running domain before and after each guest operation.
- Destroy's provider cleanup callback now receives the validated run identity
  from the lifecycle orchestrator, checks it against the CLI request, and
  derives the cleanup journal path from that recorded identity.
- Extracted `OwnershipCheckedTransport` from readiness so both readiness and
  workstation provisioning use the same pre/post ownership-check contract.
  Transport tests cover both checks around execution/upload and report ownership
  loss after a remote operation.
- `cargo fmt --all -- --check`, `cargo test --workspace --all-targets --offline`,
  and `cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
  The workspace suite includes 66 workstation tests.
- `cargo run --offline --bin skillet -- test run beezelbot --phase base --image
  fedora:latest` exited 0. Both applies succeeded and the repeat issued no
  service start/restart; Podman needed SIGKILL after its 10-second stop timeout
  during cleanup.
- No named VM or production vault/provider was used. `/dev/kvm` is absent, so
  named-VM delivery, cleanup, and production credential acceptance remain
  unverified.

### Unified typed file ownership API, 2026-10-04

- File and directory mutation APIs now accept the same `Ownership` value for
  optional named or numeric UID/GID. Local files and directories resolve named
  identities consistently, while numeric IDs do not require a matching NSS
  account. Existing metadata omitted from an ownership request is preserved.
- Recorder operations serialize this common ownership value; diagnostic
  recording schema is now version 2. Version 1 output remains historical and
  is not rewritten.
- Added local filesystem tests for numeric ownership and idempotent repeat
  apply, plus a numeric comparison test that does not perform NSS lookup.
  Existing shared mutation contract tests continue to exercise both the local
  and fake file adapters.
- `cargo fmt --all -- --check`,
  `cargo test --workspace --all-targets --offline`, and strict all-target
  Clippy passed. The beezelbot Fedora base integration exited 0: both applies
  succeeded, the repeat issued no service start/restart, and the disposable
  container was removed. Podman needed SIGKILL after its 10-second stop
  timeout during cleanup.
- Named-VM ownership/mount acceptance remains unverified because `/dev/kvm`
  is unavailable on this workstation.

### Standalone KeePassXC unlock, 2026-10-04

- Added `skillet secret unlock` to verify the configured KeePassXC database
  and establish the existing three-hour session-keyring cache without starting
  host or VM provisioning. `--database` and `--key-file` override the defaults.
- The command exits with an error if the vault opens but the cache cannot be
  established. CLI parsing tests cover the default path and both overrides;
  the vault cache tests cover the three-hour expiry and the cache result.
- `cargo fmt --all -- --check`, offline workspace all-target tests, and strict
  offline all-target Clippy passed. The beezelbot Fedora base integration
  exited 0: both applies succeeded, the repeat issued no service start/restart,
  and Podman removed the disposable container after its stop timeout.
- Interactive validation against the user's vault was not run because the
  password must be entered by the user.

### Retained clamps smoke VM readiness and fixture, 2026-10-04

- The retained `clamps-test-smoke` VM was running but its local run state was
  not marked ready. The first readiness attempt reached signed boot and base
  apply, then found `uv` missing from the cloned dotfiles Brewfile. Installed
  the missing dependency inside the disposable VM with its existing Brewfile;
  no workstation packages were installed.
- Retried `cargo run --offline --bin skillet -- test vm ready clamps smoke`;
  it passed signed boot, base apply, Homebrew bundle, and user-environment
  checks. `cargo run --offline --bin skillet -- test smoke clamps` then passed
  all fixture cases, including its reboot phase. Guest snapshots and journals
  are under `/var/lib/skillet-smoke/`; the VM remains running for inspection.
- Vault-backed Caddy/Cloudflare provisioning was not attempted. Although the
  user unlocked the vault interactively, the assistant execution process could
  not read that session keyring entry and has no interactive password input.
  No provider mutations were made by this attempt.
