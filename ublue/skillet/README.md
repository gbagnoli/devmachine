# Skillet

Idempotent host configuration for uCore. Run Cargo commands from `ublue/skillet`.
Replace `HOST` and `USER@ADDRESS` with your selected profile and SSH target.

## Build

Requires Rust/Cargo and a musl C compiler. The workspace defaults to static
`x86_64-unknown-linux-musl` binaries.

```sh
rustup target add x86_64-unknown-linux-musl
# If using this workstation's existing cross compiler:
export PATH=/opt/x86_64-linux-musl-cross/bin:$PATH
cargo build --workspace --bins
cargo build --release --workspace --bins
```

Artifact locations follow Cargo target configuration. Without `CARGO_TARGET_DIR`,
release binaries
are in `target/x86_64-unknown-linux-musl/release/`: `skillet` and
`skillet-HOST`. Both are included in prepared installation payloads.

Check installed binaries with `skillet --version` or `skillet-HOST --version`.
Bump releases in `[workspace.package].version` in `Cargo.toml`.
Both report the shared release version, 12-character Git revision and `.dirty`
for tracked changes. Source archives without Git metadata report `git.unknown`.

## Test

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run --bin skillet -- test run joey --phase base --image fedora:latest
```

The container check uses mock systemctl/Podman commands. For real runtime smoke:

```sh
cargo run --release -p skillet -- test vm create clamps smoke
cargo run --release -p skillet -- test smoke clamps --instance smoke
```

See the [VM guide](crates/vm/README.md) for prerequisites, application checks,
inspection, retries and disposal. Bootstrap regressions run from the repository
root with `bash ublue/butane/tests/bootstrap.sh`.

## Install physical hardware

1. **Inspect firmware and disks.** Boot a verified Fedora CoreOS x86_64 live ISO,
   check UEFI/Secure Boot and TPM support, and identify the destination whole
   disk by its stable by-id path. Follow [hardware preparation](../butane/HARDWARE.md#prepare).
2. **Prepare bootstrap access.** Supply an SSH public key, a fresh password-hash
   file, and the trusted ublue MOK certificate when required. The
   [hardware runbook](../butane/HARDWARE.md#prepare) gives the commands.
3. **Choose the root profile.** Use `unencrypted` for hardware without a suitable
   TPM, or `tpm` for unattended encrypted root. Read the
   [TPM and recovery guide](crates/workstation/docs/tpm.md) before choosing TPM.
4. **Prepare and write the USB.** Run `skillet install prepare` with the selected
   host, disk, access inputs and root profile. Write the generated live ISO to
   USB; follow [write and install](../butane/HARDWARE.md#write-and-install).
   Media preparation does not write the server disk.
5. **Install and finish signed bootstrap.** Boot the USB, enroll the MOK when
   using Secure Boot, run the live installer against the confirmed disk, then
   reboot. Wait for signed uCore bootstrap and verify the SSH fingerprint at
   the console before connecting. Keep local recovery access available.
6. **Prepare the vault and selected modules.** Follow
   [workstation setup](crates/workstation/README.md) and the
   [secrets checklist](../../SECRETS.md). The
   [host/module index](crates/host-profiles/README.md) links each service guide.
   Stop conflicting production identities and DNS writers before migration.
7. **Apply the host.** Remote apply builds and deploys the current
   binaries before configuring services. From the workstation:

   ```sh
   cargo run --release -p skillet -- secrets unlock
   cargo run --release -p skillet -- secrets check --host HOST --environment production
   cargo run --release -p skillet -- apply --host HOST --target USER@ADDRESS
   ```

   [Remote apply](crates/workstation/README.md#apply-a-production-host) explains
   SSH defaults and retries. [Static networking](crates/network/README.md)
   prepares profiles for boot without switching the active connection.
8. **Save recovery material.** For encrypted root, enroll the independent vault
   recovery key and header backup using the [TPM guide](crates/workstation/docs/tpm.md#production-recovery-key).
   Back up the vault off the server before relying on unattended boot.
9. **Verify and cut over.** Check mounts, services, client access and unattended
   reboot, then follow the selected host's [migration plan](../plan/README.md).
   Keep the old service/data source available until its cutover checks pass.
10. **Finalize administrative access last.** Follow
    [access finalization](crates/cli-common/README.md#administrative-access):
    force the first-login password change and require a password for sudo.

The [full hardware runbook](../butane/HARDWARE.md) contains installer and rescue
commands. Host-specific decisions and unfinished acceptance live in
[design](../design/README.md) and [plan](../plan/README.md).
