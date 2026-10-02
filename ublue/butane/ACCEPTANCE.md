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
