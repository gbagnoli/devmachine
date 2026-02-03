# butane wrapper

This is a simple compiler script that uses butane (running in podman) to
transpile the configurations to ignitions.

It has minimal support for includes: every `.bu` include will get first
transpiled to ignition, then the original source will be modified to point to
the generated ignition file. An include can *not* include other files this way.

## running

```bash
./bin/butane -s <butane_file>
```

this will generate a ignition file under a directory `ignition` (can be
controlled by passing `-o`. Without `-s` the output will go to STDOUT.
See `-h` for more

## coreos-install

```bash
./bin/coreos-install <butane_file>
```

this will use the above script to transpile the file, then download a coreos
image and finally boot a KVM machine executing the ignition file.
It supports being run inside distrobox and will execute KVM on the host.
