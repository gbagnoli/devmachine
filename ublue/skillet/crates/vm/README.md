# Disposable smoke VMs

Run Cargo commands from `ublue/skillet`. Lifecycle commands take separate `HOST`
and `INSTANCE` values. Supported templates are `clamps` and `joey`; for example,
`clamps` + `smoke` records domain `clamps-test-smoke`. Other names identify only
disposable instances of an available host, not new host recipes.

## Prerequisites

Requires Cargo, Podman, `yq`, `rg`, `ssh-keygen`, SSH and accessible `/dev/kvm`.
Native user-session libvirt needs `virsh`, `virtqemud`, `virtstoraged` and `passt`;
`virt-install` is optional. The fallback uses an existing Flatpak virt-manager
installation and QEMU extension, with its `virt-install`/QEMU user networking.
The helper installs no workstation software.

The selected backend, libvirt URI and runtime directory are recorded in the run
manifest. Native defaults to `qemu:///session`; fallback/Flatpak runtimes can use
`/run/user/UID/skvm`. Use `status` for the recorded URI rather than guessing it
from a VM name. Connect virt-manager to that same reachable socket/URI.

If Rocky QEMU reports `cannot limit core file size ...`, set `max_core = 0` in
`${XDG_CONFIG_HOME:-$HOME/.config}/libvirt/qemu.conf` and restart the **idle** user
virtqemud. Do not restart a daemon managing active work to change this setting.
TPM-specific prerequisites are in the [TPM guide](../workstation/docs/tpm.md).

## Create, provision, smoke, destroy

```sh
cargo run --release -p skillet -- test vm create clamps smoke
cargo run --release -p skillet -- test smoke clamps --instance smoke
cargo run --release -p skillet -- test vm destroy clamps smoke
```

Create includes boot/base/user readiness. Port 2201 is the default; use a free
`--port 2202` for another retained run. `--image PATH` selects a verified FCOS
qcow2; `butane/images/coreos.qcow2` is the local default when present.
The VM retains its disk after create/smoke until explicitly destroyed.

The default smoke checks a namespaced synthetic fixture, not the actual host
applications. For declared application checks on an existing ready VM:

```sh
cargo run --release -p skillet -- test vm update clamps smoke
cargo run --release -p skillet -- secrets unlock
cargo run --release -p skillet -- test vm provision clamps smoke --with-ui
cargo run --release -p skillet -- test smoke clamps --instance smoke --with-applications
```

Provision enrolls Tailscale through workstation OAuth and converges the declared application services. Repeat
provision reuses identity. Identity-protected Pi-hole needs no password;
`--rotate` affects only callers retaining native Pi-hole login.
`--with-ui` enables staging Caddy/Cloudflare lifecycle.
Opt-in [DDNS](../ddns/README.md) and [Datadog](../datadog/README.md) use
`--with-ddns` and `--with-datadog`. [SMTP-only capture](../smtp/README.md#disposable-checks)
needs neither provider delivery nor Tailscale. Production networking entries are
never delivered by disposable provisioning.

`ready` (also called at the end of smoke) reinstalls captured bootstrap binaries. After `ready` on a retained VM,
run `update` to select current code before provisioning or smoke. Smoke verifies
the recorded installed binaries; it does not replace them with the current source.

Smoke uses manifest-bound SSH settings. Optional target/port/identity overrides
must match the manifest. It builds/transfers a separate fixture and verifies
installed generic/host artifact hashes. Application mode checks selected units,
containers, network/listeners, storage/ownership, repeat apply and reboot
persistence. Failures retain private evidence and the VM for inspection.

## Inspect and update

```sh
cargo run --release -p skillet -- test vm list
cargo run --release -p skillet -- test vm list clamps
cargo run --release -p skillet -- test vm status clamps smoke
cargo run --release -p skillet -- test vm logs clamps smoke
cargo run --release -p skillet -- test vm ssh clamps smoke
cargo run --release -p skillet -- test vm reboot clamps smoke
cargo run --release -p skillet -- test vm ready clamps smoke
cargo run --release -p skillet -- test vm update clamps smoke
```

Ready reinstalls the artifacts captured at creation; update builds and verifies
current generic/host binaries. Guest paths are `/var/usrlocal/bin/skillet` and
`/var/usrlocal/bin/skillet-HOST`. `list` distinguishes unavailable runtime queries
from absent or mismatched guests. Never manually delete recovery metadata to
bypass failed cleanup.

The wrapper from `ublue/butane` is `./bin/test-vm HOST COMMAND INSTANCE`:

```sh
./bin/test-vm clamps create smoke
./bin/test-vm clamps list
./bin/test-vm list
./bin/test-vm clamps ready smoke
./bin/test-vm clamps destroy smoke
```

All operations use the same Rust lifecycle/ownership implementation. Old runs
are validated/imported explicitly; status does not rewrite them.

## Diagnostics and forwards

Private Ignition, disk, SSH key/trust and diagnostics live in the ignored
`butane/runs/NAME/` directory. Backend ownership is checked before/after guest
operations. Ready uses bounded probes and can allow up to 45 minutes per
SSH/signed-boot phase. Inspect `readiness.log`, `user-environment.log`,
`final-status.json` and journals when a phase fails.

A guest-only local endpoint can be forwarded while keeping the owned session open:

```sh
cargo run --release -p skillet -- test vm ssh clamps smoke --forward 18443:8443
```

Both ends bind localhost; repeat `--forward` for other endpoints. Port conflicts
fail rather than silently dropping a forward. Browser certificate warnings are
expected for a self-signed endpoint at localhost. A forward is not a network fence;
follow [isolated UniFi restore](../unifi/README.md#restore-and-inspect) before
importing production state.

## Other checks

The CI container runner executes base apply twice with mock systemctl/Podman:
`cargo run --bin skillet -- test run joey --phase base --image fedora:latest`.
`--inspect` opens that temporary container before cleanup. It does not establish
real runtime or reboot acceptance.

For interrupted reboot recovery on an existing ready VM:

```sh
bash integration_tests/interrupted-reboot-recovery.sh clamps smoke
```

Add `--with-applications` only to a provisioned VM. The script expects the
injected first failure, resumes readiness in a fresh process, then checks the
normal reboot cycle. For encrypted-root/cold-boot/header recovery, use the
[TPM guide](../workstation/docs/tpm.md#disposable-tpm-checks).

Destroy cleans owned external DNS/tokens/Tailscale devices before deleting the
VM/disk/key/run artifacts. It may require the vault cache; cleanup errors retain
journals for retry. An inaccessible libvirt runtime is an error, not absence.

Design: [VM lifecycle](../../../design/smoke-vms.md).
Deferred acceptance: [refactor tracker](../../../plan/SKILLET-REFACTOR.md).

## Owned guest assertions

For a guest-side acceptance executable/script, the `guest_command` example holds
this run's lock, validates UUID/disk ownership around every operation and uses
manifest-bound SSH trust. Keep credentials out of assertion output.

```sh
cargo run -p skillet_vm --example guest_command -- "$PWD/../butane" clamps homeassistant /usr/bin/true
```

`--upload ABSOLUTE_SOURCE /var/tmp/FILENAME` verifies the uploaded artifact hash.
Large application images can exceed the source FCOS disk capacity. For an existing
disposable run, `--grow-disk 64` grows its owned virtual disk to 64 GiB (never
shrinks); the guest partition and filesystem still need explicit expansion through
an owned guest assertion. This does not change physical installation storage.
