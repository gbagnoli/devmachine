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
`FORCE_HYPERLINK=1` overrides detection on terminal stdout; pipes keep plain text.

## Apply a production host

After signed bootstrap, data/identity migration preparation and the vault audit:

```sh
cargo run --release -p skillet -- apply HOST
```

Remote apply first builds and deploys both current binaries, prints their verified
versions, then configures all caller-selected services. Deployment requires this
source checkout, Cargo and the musl compiler; it honors Cargo's target directory.
Use `apply HOST --no-deploy` to reuse installed
binaries (their versions are still printed).

Deploy binaries separately, without vault access or service activation:

```sh
cargo run --release -p skillet -- deploy HOST
```

Keep temporary passwordless sudo
until provisioning/recovery operations finish. Stop old DNS writers and restored
controller identities before activating their replacements. Apply does not
migrate service data. Deployment verifies host identity before
building and again before transfer, installs each binary through a unique staged
file and atomic rename, restores its SELinux label, then checks hashes, root
ownership, mode and versions. The pair is not an atomic transaction: failed
partial deployment stops apply; retry converges the remaining binary. Service
units converge through the newly deployed guest base phase.

The positional host is mandatory and also the default SSH alias: `apply HOST`
and `deploy HOST` connect as the equivalent of `ssh HOST`. OpenSSH reads its
normal configuration, including HostName, User, Port, IdentityFile and known-host
settings; otherwise its standard defaults apply. `--target OTHER_ALIAS` or
`--target USER@ADDRESS` overrides the destination while retaining the selected
host profile. `--port`, `--identity` and `--known-hosts` override only those settings.
Guest units keep `apply --host-file /etc/skillet/host --phase PHASE`; for an
explicit local invocation use `apply HOST --local --phase PHASE`.

SSH runs noninteractively with strict host verification. Follow
[SSH-agent setup](#ssh-agent) for password-protected keys. SSH failures are
reported separately from guest identity mismatches. Verify the host fingerprint
at the server console and establish trust with SSH; unknown/changed keys are
refused.

Failure stops later phases. Repair the reported prerequisite and repeat apply.
Installed Tailscale enrollment and persistent Cloudflare child tokens are reused.
Networking preparation does not switch the live connection; follow its module
README for reboot adoption. Monitoring runs after selected workloads.

## SSH agent

Run these commands on the workstation, not inside the remote SSH session.
Check for a reachable agent and loaded keys:

```sh
ssh-add -l
```

If it cannot connect to an agent (or `SSH_AUTH_SOCK` is unset), start one in the
current shell:

```sh
eval "$(ssh-agent -s)"
```

Load the private key used for your host alias; replace the example path:

```sh
ssh-add ~/.ssh/operator_key
cargo run --release -p skillet -- apply HOST
```

Enter the SSH-key passphrase at the terminal prompt. Keep using that same shell
so Skillet inherits `SSH_AUTH_SOCK`. This unlock is separate from KeePass and its
three-hour cache. Do not start another agent when an existing one is reachable.
List configured key paths when needed (use the override destination if supplied):

```sh
ssh -G HOST | sed -n '/^identityfile /p'
```

An advertised socket alone does not establish a working agent or a loaded key.
If authentication still fails, verify the configured destination/user/key and
server authorized keys. Skillet does not automatically start an agent or unlock
private keys. For an agent you started solely for this shell, `ssh-agent -k`
stops it when finished; do not stop a shared desktop/session agent.

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
