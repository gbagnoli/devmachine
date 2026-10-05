# Clamps VM bootstrap

A clean disposable VM create passed signed boot and readiness on 2026-09-26.
The Flatpak backend on Bazzite passed create, status, and readiness on 2026-09-27.
See [ACCEPTANCE.md](ACCEPTANCE.md) for the checks performed and remaining work.

Design decisions: [disposable VM lifecycle](../design/smoke-vms.md) and
[secret storage and delivery](../design/secrets.md).

`clamps.bu` is a VM configuration. It resizes and reformats partition 4 on
`/dev/vda`, the virtio disk attached by this launcher. Do not use it as a
physical-disk install file. The host needs Podman, `yq`, `rg`, `ssh-keygen`,
and KVM. Native libvirt additionally needs `virsh`, `virtqemud`,
`virtstoraged`, and `passt`; Rust defines native VMs through `virsh`. The
Flatpak backend uses `virt-install`, QEMU user networking, and an existing
virt-manager installation with its QEMU extension.
The shared [data storage design](../design/storage.md) is implemented by
`includes/data-storage.bu`. On a fresh VM it mounts the Btrfs `data` subvolume
at `/var/lib/data` and sets rootful Podman's graphroot there. Recreate a
disposable VM made before this include to check the install-time layout.
`bin/butane` builds and stages the generic `skillet` CLI and the host-specific
`skillet-HOST` credential CLI when the input includes `includes/skillet.bu` and
the binaries were not explicitly staged under `files/`. Set `CARGO_TARGET_DIR`
to redirect Cargo output. The generated Ignition therefore contains both
executables for a normal host install. The VM launcher removes the two large
file entries from its temporary Ignition copy and delivers those same binaries
over SSH after the first boot.

The VM command selects native user-session libvirt when its tools and daemons
are available, then uses the existing Flatpak virt-manager runtime otherwise.
Flatpak libvirt daemons use `/run/user/UID/skvm` and QEMU user networking to
forward the selected localhost SSH port. The selected backend, connection URI,
and runtime directory are recorded in `runs/NAME/vm.json`. Use
`test vm status HOST INSTANCE` to inspect the selected connection. Native runs
normally use `qemu:///session`; a fallback runtime directory is also recorded
in the manifest. Check that `/dev/kvm` is accessible on the host. The helper
does not install host software.

On Rocky Linux, if QEMU startup reports `cannot limit core file size ... to
18446744073709551615`, set `max_core = 0` in
`${XDG_CONFIG_HOME:-$HOME/.config}/libvirt/qemu.conf` and restart the idle user
`virtqemud` daemon. This account had a zero core-dump hard limit; the user
session's `qemu.conf` now contains that setting. `virt-install` is optional for
the native backend.

From this directory, create and check the default disposable clamps VM with:

```bash
./bin/test-vm clamps create smoke
./bin/test-vm clamps ready smoke
```

For another disposable run, choose a unique test name and loopback SSH port:

```bash
./bin/test-vm clamps create example --port 2202
./bin/test-vm clamps ready example
```

The command form is `./bin/test-vm <host> <command> <instance>`. The host must
have a Skillet host crate; creating a VM also requires `HOST.bu`. The instance
distinguishes disposable runs and may contain lowercase letters, digits, and
hyphens. The helper records `clamps` plus `smoke` as the internal domain
`clamps-test-smoke`. Use these commands after creation:

```bash
./bin/test-vm clamps list          # available template and recorded instances
./bin/test-vm list                 # all known hosts and recorded instances
./bin/test-vm clamps status smoke  # domain and attached disks
./bin/test-vm clamps logs smoke    # bootstrap and Skillet journals
./bin/test-vm clamps ssh smoke     # interactive guest shell
./bin/test-vm clamps reboot smoke  # reboot the guest
./bin/test-vm clamps ready smoke   # rerun readiness after a reboot or repair
./bin/test-vm clamps update smoke  # build and install both current binaries
./bin/test-vm clamps destroy smoke # delete the VM, disk, test key, and run artifacts
```

`list` reports each host as `available` when both its Butane config and host
crate exist. Instance rows show the libvirt state; `missing`, `mismatch`, or
`invalid` identify runs needing inspection. `unavailable` means the recorded
runtime could not be queried; it does not establish that a guest is absent.
Rust owns VM creation, including backend selection and startup. The shell helper
keeps the established command shape and forwards each request to Skillet; the
Rust command is also available directly:

```bash
cargo run --release -p skillet -- test vm create clamps smoke
cargo run --release -p skillet -- test vm list clamps
cargo run --release -p skillet -- test vm status clamps smoke
cargo run --release -p skillet -- test vm ready clamps smoke
cargo run --release -p skillet -- test vm provision clamps smoke
cargo run --release -p skillet -- test vm destroy clamps smoke
```

All VM lifecycle commands use the Rust ownership implementation; `bin/test-vm`
is a compatibility argument adapter. Reading status does not
import or rewrite a legacy run. Disposal creates a private versioned manifest
while retaining original files until cleanup finishes. If the owned guest is
already absent, disposal still cleans recorded external resources before
removing local artifacts. A stopped/inaccessible libvirt runtime is reported
as an error; retry after making its recorded connection available. An enrolled
VM may require the normal KeePassXC unlock to remove its provider resources.
Failed cleanup retains recovery metadata. See the
[refactoring roadmap](../plan/SKILLET-REFACTOR.md) for remaining acceptance
and follow-on workstreams.

Rust builds the generic `skillet` CLI and the selected host binary from Cargo's
reported artifact paths. It prepares the run's disk and SSH key, stages and
specializes the Butane source tree, and records captured binary hashes in
`runs/NAME/`. The standalone Butane compiler builds Ignition; Rust selects and
starts native libvirt or the installed Flatpak backend, then runs readiness.
`test-vm ready` transfers
both binaries over SSH after first boot and installs them at
`/var/usrlocal/bin/skillet` and `/var/usrlocal/bin/skillet-clamps`. `ready`
uses those captured binaries on every run. The shared base unit reads the
stable host profile from `/etc/skillet/host`, so a test VM hostname can differ
from its host profile. After editing Skillet, use
`cargo run --release -p skillet -- test vm update clamps smoke` to build and
install both binaries on a retained VM. This uses Cargo's reported executables,
including a configured target directory, and verifies installed hashes and
permissions before updating `vm.json`. `provision` delivers a disposable
Pi-hole password and runs the credential-loaded full apply; `--rotate` replaces
that disposable password.
The shared uCore bootstrap script also lives in `/var/usrlocal/bin`.
Generated Ignition, SSH key, VM disk and logs stay under `runs/NAME`, which Git
ignores. The test key is private and only its public half enters Ignition.
Homebrew and the `core` dotfiles profile install after the signed clamps boot.
The dotfiles installer links `authorized_keys`; SSH also reads Ignition's key file.
`--image PATH` selects a specific FCOS qcow2;
the default is `images/coreos.qcow2` when present. Both VM backends forward
the SSH port on `127.0.0.1`; native libvirt uses passt and Flatpak uses QEMU
user networking.

Use `skillet test vm status HOST INSTANCE` and `skillet test vm logs HOST
INSTANCE` for supported VM inspection. The run manifest is the authority for
the libvirt URI and runtime directory; do not guess these values from the VM
name.

Connect with:

```bash
ssh -i runs/NAME/ssh/id_ed25519 -p PORT \
  -o UserKnownHostsFile=runs/NAME/ssh/known_hosts giacomo@127.0.0.1
```

The VM staging step grants `giacomo` passwordless sudo. The guest starts
`ucore-bootstrap.service`, which reads the host image from
`/etc/ucore-bootstrap-image` and rebases first to the unsigned clamps image
and then to the signed image. It checks the *booted* rpm-ostree deployment on
every boot. It never starts the base apply until the signed deployment boots.
A failed rebase stops without rebooting; an already pending deployment gets at
most two reboot attempts. The shared base unit runs
`/var/usrlocal/bin/skillet apply --host-file /etc/skillet/host --phase base`
with no app credentials. Clamps adds a separate full-apply unit gated on its
Pi-hole and Tailscale credentials.

`./bin/test-vm HOST ready INSTANCE` waits up to 45 minutes per SSH/signed-boot
phase with bounded probes, and checks the recorded domain UUID/disks,
noninteractive SSH and sudo, signed booted deployment, successful
Skillet unit, guest artifact SHA and metadata, enforcing SELinux, DNS,
the profile's masked units,
Homebrew, dotfiles links, and the complete `core` Brewfile bundle.
It writes `final-status.json`, `final-boot-id`,
`guest-skillet*.sha256`, `readiness.log`, and `user-environment.log` into the run
directory with mode 0600. On failure, it attempts to retain relevant journals
in those logs while preserving the original error. When ownership no longer
matches, it refuses further guest contact. `bin/test-vm-ready RUN_DIR` remains
a compatibility entry point to the same Rust implementation.
For manual diagnostics, use `test-vm HOST status INSTANCE` and
`test-vm HOST logs INSTANCE`.
The equivalent guest SSH command is:

```bash
ssh -i runs/NAME/ssh/id_ed25519 -p PORT giacomo@127.0.0.1 \
  'sudo rpm-ostree status; sudo journalctl -b -u ucore-bootstrap.service -u skillet-apply.service'
```

`create` selects `HOST.bu`, builds `skillet-HOST`, and uses port 2201 by default.
Other commands check the recorded host, domain UUID, and disk before acting.
`destroy` removes only the selected test-owned resources.

The VM and its disk remain available after readiness checks. Destroy runs with
`skillet test vm destroy HOST INSTANCE` so external enrollment/DNS cleanup and
the owned libvirt resources follow the recorded recovery journal.

Run the local bootstrap state regression checks with `bash tests/bootstrap.sh`.
They exercise mocked deployment states and do not create or modify a VM.
