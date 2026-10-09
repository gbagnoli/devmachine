# Skillet

Monitoring composition and Datadog Agent acceptance are described
in the [monitoring design](../design/monitoring.md) and
[Datadog plan](../plan/CLAMPS-DATADOG.md). Container modules currently emit
credential-free Autodiscovery labels. The optional Agent uses a separate
credential-gated apply phase.

## Datadog

For an opted-in host, create this KeePassXC entry (Password field):

- `skillet/datadog/api-key`: shared Datadog ingestion API key.

Production and test use the same key; telemetry is tagged `env:prod` or
`env:test`. Production region comes from the host caller (Clamps: `region:ftwo`);
all tests use `region:lab`. Delivery replaces template region tags.
No application key is required for ingestion. Site is public configuration in
the `datadog` default template in
[configuration-templates.json](crates/workstation/src/configuration-templates.json),
defaulting to `datadoghq.com`. Change it there for another Datadog region;
host overrides and private tags use the shared configuration-template pattern.
Move any previous environment-specific key to the shared entry. Obsolete
Datadog key/site vault paths are reported as unused; the checker never deletes them.

Deliver using `skillet secret deliver <host> datadog` with the usual SSH
identity, known-hosts, and target options. For an owned disposable VM, use
`skillet test vm provision <host> <instance> --with-datadog`. Unlock the vault
through the existing `secret unlock` workflow. Ordinary provisioning and smoke
do not enable Datadog. Historical telemetry remains after VM destruction.

For one-time monitor administration, optionally add
`skillet/datadog/monitor-setup-key` (Password field) with `monitors_read` and
`monitors_write` scopes from Datadog Organization Settings → Application Keys.
This credential stays on the workstation. Exclude `env:test` in host/service
monitor data scopes; retain coverage for legacy Chef hosts tagged `env:home`.
Changing monitor metadata tags does not exclude matching telemetry.
The all-host Agent no-data and NTP monitors were updated on 2026-10-08;
include the same test exclusion when creating new monitors.
Existing running test hosts need Datadog credential redelivery for the region
change; destroying a VM does not erase historical telemetry.

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

The smoke command reads the target, port, key and recorded host key from the
selected VM manifest. Optional `--target`, `--port`, and `--identity` values
must match that manifest; they cannot select another machine. It builds a
separate `skillet-smoke-fixture` executable from Cargo's reported artifact for
the running Skillet target and profile. Before running it, the guest verifies
the fixture transfer and confirms both installed Skillet binaries match the
ready VM manifest. Shared systemd
apply units use the generic binary and the stable host profile in
`/etc/skillet/host`. Repeated smoke runs
reset only the namespaced `/var/lib/skillet-smoke` fixture and its managed
files. The VM remains available for inspection and reruns.
Add `--with-applications` to require a provisioned VM and check the actual
services declared by that host profile. This checks service units, container
state/network/mounts, configured listeners, available service health probes,
unchanged runtime/configuration after repeat apply, and a data marker across
reboot. The default command checks the synthetic fixture only.

To exercise recovery when Skillet stops after saving the pre-reboot checkpoint,
run the scripted interruption, readiness recovery, and normal reboot cycle:

```bash
bash integration_tests/interrupted-reboot-recovery.sh clamps smoke
```

The script expects an existing, ready disposable VM. It deliberately expects
the first smoke command to exit at the injected checkpoint, verifies the
manifest is no longer `Ready`, runs readiness in a separate process, verifies
`Ready` is restored, then runs the regular smoke reboot and acceptance. Add
`--with-applications` for hosts with provisioned services. On clamps, the
current application check has a known Syncthing ownership divergence after
reboot; see the [acceptance log](../butane/ACCEPTANCE.md#interrupted-reboot-checkpoint-recovery-2026-10-05)
before enabling that option.

After editing host code, use
`cargo run --release -p skillet -- test vm update clamps smoke` to rebuild and
install the current binary. `ready` reinstalls the artifact captured when the
VM was created.

Rerun boot and base/user-environment acceptance with
`cargo run --release -p skillet -- test vm ready clamps smoke`.
Private diagnostics are retained in the run directory; see the
[VM commands](../butane/README.md) and [lifecycle design](../design/smoke-vms.md).

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
cargo build --release --target x86_64-unknown-linux-musl -p skillet -p skillet-clamps
sha256sum target/x86_64-unknown-linux-musl/release/skillet{,-clamps}
```

The generic artifact is `target/x86_64-unknown-linux-musl/release/skillet`;
the host artifact is `target/x86_64-unknown-linux-musl/release/skillet-clamps`.
The host artifact handles host-specific credential commands.

The smoke scenario checks baseline/full separation, container startup and
idempotency, configuration and dummy secret rotation, failed startup recovery,
interrupted activation, and reboot persistence. It writes state snapshots and
failure output under `/var/lib/skillet-smoke/` in the guest.
The guest needs passwordless sudo for the SSH user and access to
pull `docker.io/library/alpine:3.20`. When `--identity` is supplied, SSH stores
that VM's host key beside the private key.

## Secret setup and delivery

Start with the repository-root [secrets checklist](../../SECRETS.md) for the
entries required by each module.

The command group is `skillet secrets`; `skillet secret` remains an alias.

```bash
cargo run --release -p skillet -- secrets unlock
cargo run --release -p skillet -- secrets check
cargo run --release -p skillet -- secrets check --host clamps --environment test
```

`check` defaults to all declared host profiles and production requirements.
It reads the vault, reports missing or invalid required entry fields with
the checklist's setup instructions, and exits unsuccessfully if any are missing
or invalid. Optional and automatically generated entries need not exist.
SMTP preparation uses one entry, `skillet/smtp`: set **Username** and
**Password**, plus custom fields `host` (Mailjet: `in-v3.mailjet.com`), `port`
(`587`), and `tls` (`starttls`). The audit requires it for all declared fleet
hosts in prod and test, checks field presence/formats, and never prints values.
SMTP service delivery is still planned; the check does not authenticate to Mailjet.

Unused `skillet/` entries are reported against **all hosts and both prod/test**,
regardless of the check filter; unrelated personal entries are excluded. Unused
entries are advisory and are never deleted. No provider calls or credential
generation occur. These commands accept `--database` and `--key-file` overrides.

Audit output uses plain labels and setup instructions. Supporting terminals get
clickable web links through OSC 8; pipes and unsupported terminals get readable
link destinations. Repository-relative documentation links remain file paths.
Set `FORCE_HYPERLINK=0` to disable links, or `FORCE_HYPERLINK=1` to override
terminal detection when stdout is a terminal.

Manage durable secrets in your Syncthing-synced KeePassXC database. Use group
paths and entry titles from the [secret design](../design/secrets.md), storing
each value in its Password field. The workstation prompts for the database
password on its terminal and keeps the unlock in the Linux kernel keyring for
three hours. It does not write decrypted values to a file. Run
`cargo run --release -p skillet -- secret lock` to clear the cache early.
If the kernel denies key expiry, Skillet reports that it could not cache the
unlock and asks for the password on the next command.

To unlock the database and verify the three-hour session cache without
provisioning a host or VM, run:

```bash
cargo run --release -p skillet -- secret unlock
```

Use `--database PATH` for a non-default database or `--key-file PATH` when the
database uses a key file. The command verifies the database, then exits; it
does not change the vault. It reports an error if the password is valid but
the kernel session cache could not be established.
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
new password. Provisioning also requires the current Butane
`skillet-full-apply.service` to load both the Pi-hole and Tailscale credentials.
Skillet checks this before unlocking the vault and tells you to recreate older
test VMs that lack the Tailscale credential load.
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

For Cloudflare token issuance, create an **account-owned API token** with
**Account > API Tokens > Write** (Read also works for permission discovery)
and **Zone > Zone > Read**, with Zone Resources restricted to the configured
test and/or production zones. Skillet reads the account ID from the selected
zone and uses the account-token endpoints to create, list, and revoke child
tokens. The issuer does not need DNS Write. Save it as
`skillet/cloudflare/token-creator`. Skillet discovers the Cloudflare
permission-group IDs at runtime, then creates zone-scoped child tokens with
Zone Read and DNS Write. If the issuer is already stored in the desktop
keyring, move the value into KeePassXC and verify the saved entry before
removing the old copy.

### Caddy UI credentials

Caddy proxies host-declared UIs on port 443 over Tailscale. Prepare these
KeePassXC entries in their Password fields:

| Group path | Entry title | Value |
| --- | --- | --- |
| `skillet/environments/<environment>/dns` | `ui-domain` | Optional relative prefix; defaults to `ui` when absent |
| `skillet/environments/<environment>/dns` | `cloudflare-zone-id` | Required Zone ID for `prod` or `test` |
| `skillet/cloudflare` | `token-creator` | Workstation token issuer |

Select `production` or `test` for the CLI environment; use `prod` or `test` as
the KeePassXC environment group name. Copy the Zone ID from the
selected zone's Cloudflare Overview page. Skillet fetches the zone's domain
and always appends it to the relative prefix. For zone `example.com`, an
absent entry produces `ui.example.com`; `ui.whatever` produces
`ui.whatever.example.com`. An empty entry is invalid. Replace any former
full-domain value with a relative prefix, or remove the entry to use `ui`.
Full-domain values are not detected or reused as absolute names: they too
are appended to the zone. Do not copy a full domain from the old
`skillet/dns/smoke-ui-zone` entry. The previous `cloudflare/zone` entry contained
a zone name; populate `dns/cloudflare-zone-id` with the actual ID.
Clean up any existing disposable UI deployment with the previous CLI and
configuration before migrating its prefix; cleanup verifies the recorded
resolved namespace and refuses a changed configuration.

The CLI reads these environment paths directly. Persistent Caddy delivery
creates and saves a missing host ACME token at
`skillet/environments/<environment>/hosts/<host>/cloudflare/acme-token`, then
reconciles the host's Tailscale A/AAAA records and UI CNAMEs. Smoke VMs only do
this when explicitly provisioned with `--with-ui`; they receive a 12-hour
zone-scoped token and `test vm destroy` removes its owned records and token.
Do not create a manual smoke VM ACME token. See the
[generic UI plan](../plan/GENERIC-PRIVATE-UIS.md) and
[Cloudflare lifecycle design](../design/cloudflare-ui-lifecycle.md).

### Tailscale setup for Skillet

Clamps runs `tailscale/tailscale` as a host-network container, matching Chef
on rupik, calculon, and boxy. Its state persists under
`/var/lib/data/tailscale`. Skillet reads the workstation OAuth client from
KeePassXC, mints a one-use enrollment key, and sends only that key to the host
as an encrypted systemd credential and Podman secret. Smoke VM destruction
removes the matching `tag:skillet-smoke` device before deleting the VM. This
requires `curl` on the workstation. See the
[private UI design](../design/private-ui-access.md).

1. In the Tailscale admin console, open **Access controls** and ensure the
   policy defines `tag:skillet-provisioner`, `tag:skillet-server`, and
   `tag:skillet-smoke`. Set `tagOwners` so the provisioner tag owns the server
   and smoke tags. Give smoke devices only smoke access.
2. In **Trust credentials**, create an OAuth client restricted to
   `tag:skillet-provisioner`, with both **Keys > Auth Keys: Write** (`auth_keys`)
   and **Devices > Core: Read/Write** (`devices:core`) scopes. The first mints
   enrollment keys; the second lets Skillet find and remove its tagged smoke
   device. Do not grant DNS-management scopes. Keys are one-use, preauthorized,
   non-ephemeral, and expire after one hour if unused.
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
4. Keep the test and production UI domain and Cloudflare Zone ID in their
   respective KeePassXC environment entries. Skillet validates each domain
   against the exact zone ID before it creates credentials or DNS records. No
   delegated subzone or Tailscale split DNS configuration is required.
5. For a disposable VM, run `cargo run --release -p skillet -- test vm provision clamps smoke`.
   Skillet prompts to unlock KeePassXC when its three-hour kernel cache is
   empty, enrolls the VM, then provisions Pi-hole. Repeat the command to reuse
   the Tailscale identity and Pi-hole credential. Add `--with-ui` to also mint
   the disposable Cloudflare credential, publish its tailnet DNS records, and
   activate Caddy with the test ACME staging issuer. Dispose of the VM with
   `cargo run --release -p skillet -- test vm destroy clamps smoke`. If
   Tailscale cleanup fails, destruction stops and the VM metadata remains for
   retry. UI provisioning also probes each configured hostname from the VM's
   loopback address and requires Caddy's explicit 403 denial response. This
   checks the source-address rule; it does not replace a test from a separate
   network outside the tailnet. Keep the OAuth client available for cleanup.
6. For production, deliver the Pi-hole credential first, then Tailscale:

   ```bash
   cargo run --release -p skillet -- secret deliver clamps tailscale \
     --target giacomo@clamps --identity /path/to/ssh-key \
     --known-hosts /path/to/known_hosts
   ```

   This mints a production-tagged key; full apply then configures both
   services. Preserve the encrypted KeePassXC entries so another workstation
   can reprovision credentials.

Cloudflare credentials and DNS records are removed before the smoke Tailscale
device and VM are destroyed. Keep the Cloudflare token creator on the
workstation; Tailscale enrollment needs no Cloudflare or Tailscale DNS
permissions. Staging ACME and HTTPS acceptance passed on 2026-10-02; production
renewal and access checks from outside the tailnet remain deferred. See the
[VM lifecycle](../design/smoke-vms.md).

### Optional Cloudflare DDNS

The selected host profile must declare DDNS. Clamps currently enables it.
In KeePassXC's Password fields, set:

- `skillet/environments/prod/dns/cloudflare-zone-id`: the existing environment
  zone ID.
- `skillet/environments/prod/hosts/<host>/cloudflare/ddns-dns-name`: one
  approved relative record name, without the zone suffix.
- `skillet/cloudflare/token-creator`: the account token described above.

The clear JSON policy is in
`crates/workstation/src/configuration-templates.json`. Its shared DDNS template
reads the record name through a typed `$secret` reference. A host-specific
template in that catalog replaces the complete shared template when needed.
Change template policy in source and rebuild Skillet; do not store the JSON
configuration in KeePassXC.

Skillet creates `skillet/environments/prod/hosts/<host>/cloudflare/ddns-token`
automatically. Close KeePassXC before delivery. With the prior DNS writer stopped
and the selected record set approved, run:

```bash
cargo run --release -p skillet -- secret deliver clamps ddns \
  --target giacomo@clamps --identity /path/to/ssh-key \
  --known-hosts /path/to/known_hosts
```

To adopt an existing public A record, set `takeover_existing` to `true` in the
source template after reviewing it. Conflicting records and other Skillet owners
remain protected. Repeat delivery reuses the child token; edit its vault entry
deliberately and redeliver for rotation. On the host,
`systemctl start skillet-ddns-apply.service` reuses the encrypted credential.
Use `test vm provision <host> <instance> --with-ddns` for disposable DDNS. Its
record/token ownership is cleaned up by `test vm destroy`; do not use direct
`secret deliver` with test policy. See the
[DDNS design](../design/cloudflare-ddns.md) and
[remaining lifecycle plan](../plan/CLAMPS-CLOUDFLARE-DDNS.md).

## Development checks

- Workspace checks: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace --all-targets`.
- Container integration: `cargo run --bin skillet -- test run beezelbot
  --phase base --image fedora:latest` (also run for `clamps` after changing
  shared container behavior).
- Bootstrap state regressions: `bash ublue/butane/tests/bootstrap.sh` from the
  repository root.
- ShellCheck covers `*.sh` plus the extensionless scripts named in the CI
  workflow, including VM and workstation helpers.
- `apply --record PATH` writes version 2 YAML with host, overall result, and
  per-operation outcomes. It stores no content, hashes, secret values, or raw
  effect errors, and is written after failed applies as well. Older unversioned
  operation arrays remain diagnostic artifacts and are never loaded as state.

- **Error Handling**: Use `thiserror` in library crates; `anyhow` is reserved for CLI binaries. No `unwrap()` or `expect()` in library code.
- **Idempotency**: All modules must ensure system state idempotently.
- **System Interactions**: Prioritize Rust crates (e.g., `zbus`, `users`) over shelling out to system commands.
- **Linting**: Run `cargo clippy --workspace --all-targets -- -D warnings`; the workspace enables pedantic lints with documented exceptions.
