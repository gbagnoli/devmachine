# Skillet

Skillet is a Rust tool for idempotent host configuration management.

Design decisions: [secret storage and delivery](../design/secrets.md) and
[disposable VM lifecycle](../design/smoke-vms.md).

## Building

Skillet builds fully static x86_64 musl binaries by default
(see `.cargo/config.toml`).

One-time setup:
```bash
rustup target add x86_64-unknown-linux-musl
```

### Development Build
To build the workspace for development, use the standard cargo command:
```bash
cargo build
```

### Production Build
For optimized production builds, use the `--release` flag:
```bash
cargo build --release
```
The per-host binaries land at
`target/x86_64-unknown-linux-musl/release/skillet-<hostname>`
(e.g. `skillet-clamps`). Verify with `file`, which should report
"statically linked".

## Running

The tool provides an `apply` command to execute configuration. `apply` defaults
to the full host and requires its application credentials. Use `--phase base`
to converge only the shared host baseline before credentials are available.

- **Agent Mode**: Run generically on a host:
  ```bash
  ./target/x86_64-unknown-linux-musl/debug/skillet apply --host clamps --phase base
  ```
- **Host-Specific Configuration**: If a host-specific binary has been built (e.g., `skillet-beezelbot`), you can use it to apply specific configurations.

## Disposable VM smoke check

The CI command `skillet test run beezelbot --phase base --image fedora:latest`
runs the selected apply phase twice in a uniquely named, temporary Podman
container. It builds the selected host binary, checks both applies succeed,
and checks the second apply does not start or restart a service. `clamps` is
also supported. This sandbox uses mock `systemctl` and `podman` executables.
The phase defaults to `base`, since full apply may require a host's persistent
data mount and runtime credentials.
`--inspect` opens the container before cleanup.

For real systemd/Podman and reboot coverage, create a disposable VM first:

```bash
cargo run --release -p skillet -- test vm create clamps smoke
```

This provisions `clamps-test-smoke` on `giacomo@127.0.0.1:2201`, creates its SSH
key, and waits for the guest to pass `test-vm clamps ready smoke`. The same command
accepts another host when its `HOST.bu` and `skillet-HOST` are available.
Use `cargo run --release -p skillet -- test vm list` to see host availability
and recorded instances, or add a host to list only its instances.
Then run the clamps smoke scenario:

```bash
cargo run --release -p skillet -- test smoke clamps
```

The smoke command defaults to that target, port, and generated key. Override
them with `--target`, `--port`, and `--identity` when using a separately
provisioned VM. It takes the generic binary from the running executable and
uses the `skillet-clamps` binary installed by VM creation. Repeated smoke runs
reset only the namespaced `/var/lib/skillet-smoke` fixture and its managed
files. The VM remains available for inspection and reruns.
After editing host code, use
`cargo run --release -p skillet -- test vm update clamps smoke` to rebuild and
install the current binary. `ready` reinstalls the artifact captured when the
VM was created.

When finished, remove the disposable VM, disk, SSH key, and run artifacts:

```bash
cargo run --release -p skillet -- test vm destroy clamps smoke
```

The VM helpers require the host setup documented in the
[Butane README](../butane/README.md). Smoke requires the Skillet workspace and
Cargo. The VM fixture controls its own image.

Build both static binaries from this directory if you want to install the host
binary separately:

```bash
cargo build --release -p skillet-clamps -p skillet
sha256sum target/x86_64-unknown-linux-musl/release/skillet-clamps
```

The host artifact is `target/x86_64-unknown-linux-musl/release/skillet-clamps`.
Supply that artifact to the guest, then invoke `skillet-clamps apply --phase base`.

The smoke scenario checks baseline/full separation, container startup and
idempotency, configuration and dummy secret rotation, failed startup recovery,
interrupted activation, and reboot persistence. It writes state snapshots and
failure output under `/var/lib/skillet-smoke/` in the guest.
The guest needs passwordless sudo for the SSH user and access to
pull `docker.io/library/alpine:3.20`. When `--identity` is supplied, SSH stores
that VM's host key beside the private key.

## Secret setup and delivery

Manage durable secrets in your Syncthing-synced KeePassXC database. Use group
paths and entry titles from the [secret design](../design/secrets.md), storing
each value in its Password field. The workstation prompts for the database
password on its terminal and keeps the unlock in the Linux kernel keyring for
three hours. It does not write decrypted values to a file. Run
`cargo run --release -p skillet -- secret lock` to clear the cache early.
If the kernel denies key expiry, Skillet reports that it could not cache the
unlock and asks for the password on the next command.
Skillet looks for `$XDG_DATA_HOME/skillet/secrets.kdbx`, falling back to
`$HOME/.local/share/skillet/secrets.kdbx`. Point that location at your
Syncthing-synced database, for example:

```bash
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/skillet"
ln -s /path/to/your/synced-vault.kdbx \
  "${XDG_DATA_HOME:-$HOME/.local/share}/skillet/secrets.kdbx"
```

The vault-dependent command reports the missing path before asking for its
password. `--database PATH` overrides the default when needed. Disposable VM
provisioning generates its own test value and does not require a vault.

After creating and preparing a disposable clamps VM, deliver its generated
test password and run the full host apply:

```bash
cargo run --release -p skillet -- test vm provision clamps smoke
```

The password stays encrypted on the VM. Repeat this command after a reboot to
reuse it; use `--rotate` to replace it explicitly. A fresh VM starts with a
new password. The command requires the `skillet-full-apply.service` supplied
by current Butane, so recreate older test VMs made before that unit existed.
If an existing VM credential is corrupt, use `--rotate` to replace only that
disposable value.

For a production clamps host, deliver over an SSH connection whose host key
you have already recorded:

```bash
cargo run --release -p skillet -- secret deliver clamps pihole \
  --target giacomo@clamps --identity /path/to/ssh-key \
  --known-hosts /path/to/known_hosts
```

Add `--key-file /path/to/keyfile` if the database uses one. The value is
encrypted by the installed `skillet-clamps credential install` command on the
host and loaded by `skillet-full-apply.service`; later
`systemctl start skillet-full-apply.service` reuses it. If full apply
reports a missing or undecryptable credential, check the host's encrypted
credential file and redeliver the KeePassXC entry. If the entry is absent in
an existing vault and the host has no encrypted credential, Skillet creates
`skillet/hosts/clamps/pihole/web-password` and delivers it. Keep KeePassXC
closed during creation and resolve Syncthing conflict copies before retrying.
If the host already has an encrypted credential or Podman secret, restore the
original vault entry.
Redeliver the same entry after moving to a new workstation; use a deliberate
vault edit and redelivery for rotation. The production TPM binding decision
remains in the [secret design](../design/secrets.md).

For Cloudflare token issuance, select **Create additional tokens** in the
Cloudflare dashboard, with **User > API Tokens > Edit** and no extra
permissions. Save it as `skillet/cloudflare/token-creator`. If already stored
in the desktop keyring, move that value into KeePassXC and verify the saved
entry before removing the old copy. The current smoke command does not use it.

### Tailscale setup for Skillet

Skillet will run the official `tailscale/tailscale` container with
`Network=host`, matching the current Chef configuration on rupik, calculon,
and boxy. This gives Tailscale a normal host interface for host-level routing
and port forwarding. Persist its state under `/var/lib/data/tailscale`. Do not
put the OAuth client secret or a reusable auth key on the host. See the
[private UI design](../design/private-ui-access.md) for the network decision.

1. In the Tailscale admin console, open **Access controls** and ensure the
   policy defines separate tags for production servers and smoke VMs, such as
   `tag:skillet-server` and `tag:skillet-smoke`. Give each tag only the access
   that role needs. Make a provisioning tag the owner of those tags, following
   the existing `tagOwners` policy pattern; do not give smoke VMs production
   server permissions.
2. Open **Trust credentials** and create an **OAuth** credential for Skillet.
   Grant **Keys > Auth Keys: Write** (`auth_keys`) and restrict it to the
   provisioning tag from step 1. This lets Skillet mint a tagged enrollment
   key per target. Keys should be one-use, preauthorized, and non-ephemeral so
   a retained smoke VM stays registered through shutdowns. Until automated
   device cleanup is implemented, remove a destroyed VM from **Machines** in
   the admin console; revoking its auth key does not remove an enrolled device.
   Automated cleanup will require a separately reviewed `devices:core` scope.
3. Copy the OAuth **Client ID** and **Client secret** from the creation page.
   In KeePassXC create two entries, putting each value in its **Password**
   field:

   | Group path | Entry title | Password value |
   | --- | --- | --- |
   | `skillet/tailscale` | `provisioner-client-id` | OAuth Client ID |
   | `skillet/tailscale` | `provisioner-client-secret` | OAuth Client secret |

   The ID is not secret, but storing both this way lets Skillet use its
   Password-field lookup for the complete credential. The client secret can
   mint new auth keys, so treat it as the workstation-side master credential.
   Never put it in a VM, Quadlet, environment file, or repository. Close
   KeePassXC before Skillet edits the database.
4. In the Tailscale admin console, open **DNS** and leave **Override DNS
   servers** off. Under **Nameservers**, choose **Add nameserver > Custom**.
   Add the numeric Tailscale IPv4 address of each Pi-hole that is reachable
   over Tailscale, enable **Restrict to search domain**, and enter the private
   UI zone. Add a resolver only after it is reachable and serves the same
   records as the others. Clients may query resolvers in any order.
5. Configure production and smoke-test UI zones outside this public repository.
   Use a restricted nameserver for each zone only when its Pi-hole resolver has
   the matching local records. Once clamps and its Pi-hole are enrolled, add
   identical production records to every configured Pi-hole, using the form
   `<service>.<host>.<production-private-zone>` and resolving to that host's
   Tailscale address. Keep smoke-test records under a separate zone and point
   them at the disposable VM's Tailscale address. Caddy obtains trusted
   certificates through Cloudflare DNS-01; UI A/AAAA records do not need to be
   public.

The Tailscale vault entries and split DNS are not consumed/configured by
Skillet yet. This guide prepares the tailnet and vault for that milestone;
Skillet support and disposable-VM validation remain required.

## Development checks

- **Error Handling**: Use `thiserror` in library crates; `anyhow` is reserved for CLI binaries. No `unwrap()` or `expect()` in library code.
- **Idempotency**: All modules must ensure system state idempotently.
- **System Interactions**: Prioritize Rust crates (e.g., `zbus`, `users`) over shelling out to system commands.
- **Linting**: Run `cargo clippy --workspace --all-targets -- -D warnings`; the workspace enables pedantic lints with documented exceptions.
