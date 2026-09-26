# Clamps VM bootstrap

Implementation checkpoint: source validation passes; a complete VM boot and
runtime acceptance are still pending. See [ACCEPTANCE.md](ACCEPTANCE.md).

`clamps.bu` is a VM configuration. It resizes and reformats partition 4 on
`/dev/vda`, the virtio disk attached by this launcher. Do not use it as a
physical-disk install file. The host needs native libvirt on `qemu:///session`,
`virt-install`, `passt`, Podman, `yq`, `ssh-keygen`, and KVM.

From this directory, the simple clean install command is:

```bash
./bin/coreos-install clamps.bu
```

It refuses an existing `clamps` domain or `runs/clamps` directory. For
independent disposable runs, use a unique name and loopback SSH port:

```bash
./bin/coreos-install --name clamps-test-example --ssh-port 2201 clamps.bu
./bin/clamps-ready runs/clamps-test-example
```

The launcher builds the current static `skillet-clamps` host binary, or accepts
`--artifact PATH` to use a specific binary. It records its SHA256 in `runs/NAME/skillet.sha256`, copies
the binary into the generated Ignition, and delivers it at
`/var/lib/skillet/skillet-clamps`. Generated Ignition, SSH key, VM disk and logs
stay under `runs/NAME`, which Git ignores. The test key is private and only its
public half enters Ignition. `--image PATH` selects a specific FCOS qcow2;
the default is `images/coreos.qcow2` when present. The VM uses passt with an
inbound forward bound to `127.0.0.1`.

`bin/clamps-vm` starts a separate native user libvirt daemon with
`XDG_RUNTIME_DIR=/run/user/UID/clamps-native-libvirt`. This keeps the native
daemon in the same filesystem view as generated artifacts when Flatpak
virt-manager has its own session daemon. For manual virsh inspection of a
helper-created VM, set that environment variable on each virsh invocation.

Connect with:

```bash
ssh -i runs/NAME/ssh/id_ed25519 -p PORT \
  -o UserKnownHostsFile=runs/NAME/ssh/known_hosts core@127.0.0.1
```

The `core` account has passwordless sudo on FCOS. The guest starts
`clamps-bootstrap.service`, which rebases first to the unsigned clamps image
and then to the signed image. It checks the *booted* rpm-ostree deployment on
every boot. It never starts the base apply until the signed deployment boots.
A failed rebase stops without rebooting; an already pending deployment gets at
most two reboot attempts. The base unit runs
`/var/lib/skillet/skillet-clamps apply --phase base` with no app credentials.

`./bin/clamps-ready runs/NAME` waits up to 45 minutes, checks the expected
domain/disk, noninteractive SSH and sudo, signed booted deployment, successful
Skillet unit, guest artifact SHA, enforcing SELinux, DNS and masked resolved.
It writes `domain.txt`, `disks.txt`, `final-status.json`, `final-boot-id`,
`guest-skillet.sha256`, and `readiness.log` into the run directory. On timeout,
it writes `failure.log` with the current deployment and relevant journals.
For manual diagnostics:

```bash
virsh -c qemu:///session dominfo NAME
virsh -c qemu:///session domblklist NAME
ssh -i runs/NAME/ssh/id_ed25519 -p PORT core@127.0.0.1 \
  'sudo rpm-ostree status; sudo journalctl -b -u clamps-bootstrap.service -u skillet-apply.service'
```

`bin/clamps-vm` is a constrained source-tree helper for test domains named
`clamps-test-*` under the ignored `butane/runs` directory. It offers
`create|status|ssh|logs|reboot|destroy`; `destroy` checks the recorded domain
UUID and disk before removing test-owned resources. Its source-tree path is
mutable, so an enduring elevated approval requires reviewed, frozen helper
code and executable dependencies installed outside writable roots.

The VM and its disk remain available after readiness checks. Cleanup is
explicit: inspect the domain and disk, then call
`./bin/clamps-vm destroy clamps-test-example` for a helper-created run. For a
plain launcher run, shut down and undefine the named test domain with virsh,
then remove only its recorded `runs/NAME` directory after checking `domblklist`.

Run the local bootstrap state regression checks with `bash tests/bootstrap.sh`.
They exercise mocked deployment states and do not create or modify a VM.
