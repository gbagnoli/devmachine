# Workstation provisioning

Run Cargo commands from `ublue/skillet`; replace `HOST` and `USER@ADDRESS`.

## Vault setup

Use a KeePassXC KDBX4 database at `$XDG_DATA_HOME/skillet/secrets.kdbx`, defaulting
to `~/.local/share/skillet/secrets.kdbx`. A symlink can point to a synced vault:

```sh
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/skillet"
ln -s /path/to/synced-vault.kdbx \
  "${XDG_DATA_HOME:-$HOME/.local/share}/skillet/secrets.kdbx"
cargo run --release -p skillet -- secrets unlock
cargo run --release -p skillet -- secrets check --host HOST --environment production
```

Use group path + entry title from the [secret checklist](../../../../SECRETS.md).
Unless documented otherwise, values belong in Password, not Username.
CLI environments are `production`/`test`; vault environment groups are `prod`/`test`.
`--database PATH` and `--key-file PATH` override vault inputs.
Missing vault files fail before the password prompt.

Unlock uses a terminal prompt and caches the password in the Linux kernel
keyring for three hours. Run `skillet secrets lock` to clear it early. If kernel
expiry is denied, unlock reports that caching failed. Passwords must never be
passed through chat, command arguments or environment variables.

Close KeePassXC before Skillet may create/update entries. Atomic saves require
a writable vault **and parent directory**. Preserve service ownership on synced
stores; resolve Syncthing conflict copies before retrying.

`secrets check` reports required missing/invalid fields and advisory unused
`skillet/` entries. Required checks follow the selected hosts/environment;
unused checks cover all declared hosts and both environments. It prints no
values and makes no provider calls. Optional/generated entries may be absent.
Terminal links use OSC 8 when supported; `FORCE_HYPERLINK=0` disables them.

## Apply a production host

After signed bootstrap, data/identity migration preparation and the vault audit:

```sh
cargo run --release -p skillet -- apply --host HOST --target USER@ADDRESS
```

This configures all caller-selected services. Keep temporary passwordless sudo
until provisioning/recovery operations finish. Stop old DNS writers and restored
controller identities before activating their replacements. Apply does not
migrate service data or upload replacement binaries; installed guest binaries
and phase units must already match the profile.

SSH defaults to port 22, `~/.ssh/known_hosts`, and OpenSSH identity/configuration
and agent selection. First verify the fingerprint at the server console and
establish trust with SSH. Unknown/changed keys are refused. Override with
`--port`, `--identity /absolute/key`, or `--known-hosts /absolute/known_hosts`.
A protected key must already be available to noninteractive SSH.

Failure stops later phases. Repair the reported prerequisite and repeat apply.
Installed Tailscale enrollment and persistent Cloudflare child tokens are reused.
Networking preparation does not switch the live connection; follow its module
README for reboot adoption. Monitoring runs after selected workloads.

## Deliver one service

```sh
cargo run --release -p skillet -- secrets deliver HOST SERVICE --target USER@ADDRESS
```

Delivery stages by default. Add `--activate` to start its consumer. Pi-hole and
Tailscale share full apply; activating either can configure other declared
applications. Caddy staging also issues/reuses tokens and reconciles DNS.
For test policy, use an [owned VM](../vm/README.md), not manual production delivery.

The host stores encrypted credentials under
`/etc/credstore.encrypted/skillet/<credential>.cred`. Redeliver from the same
vault after changing workstations; restore existing entries before reprovisioning
an existing host. Deliberate vault edits plus redelivery perform rotation.

## Other guides

- [Cloudflare issuer and zone setup](docs/cloudflare.md)
- [TPM profiles and recovery](docs/tpm.md)
- [Host and module index](../host-profiles/README.md)
- [Configuration-template format](../../../design/configuration-templates.md)

Designs: [secret delivery](../../../design/secrets.md),
[host application](../../../design/host-application.md).
