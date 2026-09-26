# Clamps VM bootstrap

A clean disposable VM create passed signed boot and readiness on 2026-09-26.
See [ACCEPTANCE.md](ACCEPTANCE.md) for the checks performed and remaining work.

`clamps.bu` is a VM configuration. It resizes and reformats partition 4 on
`/dev/vda`, the virtio disk attached by this launcher. Do not use it as a
physical-disk install file. The host needs native libvirt on `qemu:///session`,
`virt-install`, `passt`, Podman, `yq`, `rg`, `ssh-keygen`, and KVM.

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
./bin/test-vm clamps destroy smoke # delete the VM, disk, test key, and run artifacts
```

`list` reports each host as `available` when both its Butane config and host
crate exist. Instance rows show the libvirt state; `missing`, `mismatch`, or
`invalid` identify runs needing inspection. The Skillet wrapper runs both
create and ready:

```bash
cargo run --release -p skillet -- test vm create clamps smoke
cargo run --release -p skillet -- test vm list clamps
cargo run --release -p skillet -- test vm destroy clamps smoke
```

The helper calls `coreos-install`, which builds the current static
`skillet-clamps` host binary. The lower-level launcher accepts `--artifact PATH`
to use a specific binary. It records its SHA256 in
`runs/NAME/skillet.sha256`. To keep the QEMU `fw_cfg` Ignition payload small,
the VM config omits this multi-megabyte binary; `test-vm ready` transfers it
over SSH after first boot and installs it at `/var/usrlocal/bin/skillet-clamps`.
The shared uCore bootstrap script also lives in `/var/usrlocal/bin`: Fedora CoreOS labels
that persistent directory `bin_t`, allowing systemd to execute these files
with SELinux enforcing.
Generated Ignition, SSH key, VM disk and logs stay under `runs/NAME`, which Git
ignores. The test key is private and only its public half enters Ignition.
Homebrew and the `core` dotfiles profile install after the signed clamps boot.
The dotfiles installer links `authorized_keys`; the SSH configuration also
reads Ignition's key file, so the generated VM key remains usable.
`--image PATH` selects a specific FCOS qcow2;
the default is `images/coreos.qcow2` when present. The VM uses passt with an
inbound forward bound to `127.0.0.1`.

`bin/test-vm` starts a separate native user libvirt daemon with
`XDG_RUNTIME_DIR=/run/user/UID/skillet-test-libvirt`. This keeps the native
daemon in the same filesystem view as generated artifacts when Flatpak
virt-manager has its own session daemon. For manual virsh inspection of a
helper-created VM, set that environment variable on each virsh invocation.

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
most two reboot attempts. The base unit runs
`/var/usrlocal/bin/skillet-clamps apply --phase base` with no app credentials.

`./bin/test-vm HOST ready INSTANCE` waits up to 45 minutes, checks the expected
domain/disk, noninteractive SSH and sudo, signed booted deployment, successful
Skillet unit, guest artifact SHA, enforcing SELinux, DNS, masked resolved,
Homebrew, dotfiles links, and the complete `core` Brewfile bundle.
It writes `domain.txt`, `disks.txt`, `final-status.json`, `final-boot-id`,
`guest-skillet.sha256`, `readiness.log`, and `user-environment.log` into the run
directory. On timeout, it writes `failure.log` or
`user-environment-failure.log` with relevant journals.
For manual diagnostics, use `test-vm HOST status INSTANCE` and
`test-vm HOST logs INSTANCE`.
The equivalent guest SSH command is:

```bash
ssh -i runs/NAME/ssh/id_ed25519 -p PORT giacomo@127.0.0.1 \
  'sudo rpm-ostree status; sudo journalctl -b -u ucore-bootstrap.service -u skillet-apply.service'
```

`bin/test-vm` is a constrained source-tree helper that stores its runs under
the ignored `butane/runs` directory. `create` infers
`HOST.bu`, builds `skillet-HOST`, and uses port 2201 by default. Other commands check the
recorded host, domain UUID, and disk before acting. `destroy` removes only
the selected test-owned resources. Its source-tree path is
mutable, so an enduring elevated approval requires reviewed, frozen helper
code and executable dependencies installed outside writable roots.

The VM and its disk remain available after readiness checks. Cleanup is
explicit: inspect the domain and disk, then call
`./bin/test-vm clamps destroy example` for a helper-created run. For a
plain launcher run, shut down and undefine the named test domain with virsh,
then remove only its recorded `runs/NAME` directory after checking `domblklist`.

Run the local bootstrap state regression checks with `bash tests/bootstrap.sh`.
They exercise mocked deployment states and do not create or modify a VM.
