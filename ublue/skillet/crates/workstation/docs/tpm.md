# TPM-encrypted root and recovery

Run Cargo commands from `ublue/skillet`. Both `unencrypted` (default) and `tpm`
root profiles are supported. The TPM profile uses native LUKS2/Clevis SHA256 PCR 7
beneath Btrfs, requires UEFI, Secure Boot and TPM 2, and never silently downgrades.
Normal unlock needs no network. Its protection scope is encrypted-disk
loss/removal; it does not authenticate every kernel/initramfs boot component.

## Physical installation

Follow [hardware preparation](../../../../butane/HARDWARE.md#prepare). Supply
`--root-profile tpm` and a trusted `--mok-certificate` to `skillet install prepare`.
Enroll the required ublue MOK before the first encrypted installed boot. Have an
independent recovery path before changing Secure Boot/MOK policy or TPM bindings.
For plain root use `--root-profile unencrypted`; keep the VM `/dev/vda` template
separate from the physical whole-disk by-id selection.

## Production recovery key

Before access finalization, while noninteractive sudo is still available:

```sh
cargo run --release -p skillet -- secrets root-recovery-key HOST --target USER@ADDRESS
```

Normal [vault and SSH defaults](../README.md) apply; `--database`, `--key-file`,
`--identity` and `--known-hosts` provide overrides. The writable vault and parent
directory hold an atomic replacement, lock and encrypted backup.

Skillet saves an independent printable passphrase before enrollment and verifies
it against the root volume. Password at
`skillet/hosts/<host>/storage/root-recovery-key` is the value for the LUKS recovery
prompt. The protected `root-luks.header` attachment is the verified header backup.
Repeated commands verify the retained key and refresh changed headers without
rotating the passphrase. A different volume is rejected.

Back up the vault off the host. After any keyslot changes, rerun the command to
refresh the header archive. Recover the Password/attachment from that independent
backup when needed. Header restoration is destructive rescue work: verify volume
identity and never restore onto a running root filesystem. A header is not a
service-data backup.

## Disposable TPM checks

Requires native libvirt 10.10+, swtpm 0.7+, KVM and q35 UEFI Secure Boot firmware
with enrolled keys. Creation checks capabilities; no host packages are installed.
Choose a free port and a fresh instance; a retained root profile cannot change.

```sh
cargo run --release -p skillet -- test vm create clamps encrypted --root-profile tpm --port 2202
cargo run --release -p skillet -- test vm encryption-check clamps encrypted
cargo run --release -p skillet -- test vm destroy clamps encrypted
```

`encryption-check` uses its own recovery key/header under the private run's
`recovery/` directory, verifies recovery and cold-boot persistence, and requires
exactly two slots. Disposal deletes those artifacts. It needs no vault/provider.

To archive a separate independent key in KeePassXC instead, use
`skillet test vm recovery-key HOST INSTANCE`. Its entry is
`skillet/environments/test/hosts/<host>/instances/<instance>/storage/root-recovery-key`.
Vault archives survive disposal; remove an old instance's archive before reusing
its name for a new volume. Use a fresh VM for the separate two-slot encryption
acceptance rather than combining enrollment workflows.

## TPM-loss recovery driver

Use fresh source/clone names, free ports and a base-only VM without provider
credentials. Keep other smoke instances separate:

```sh
cargo run --release -p skillet -- test vm create clamps header-recovery --root-profile tpm --port 2202
cargo run --release -p skillet -- test vm recovery-key clamps header-recovery
cargo run --release -p skillet_workstation --example tpm_recovery -- ../butane clamps header-recovery tpm-loss 2203 clone
cargo run --release -p skillet_workstation --example tpm_recovery -- ../butane clamps header-recovery tpm-loss 2203 unlock
cargo run --release -p skillet_workstation --example tpm_recovery -- ../butane clamps header-recovery tpm-loss 2203 rebind
cargo run --release -p skillet_workstation --example tpm_recovery -- ../butane clamps header-recovery tpm-loss 2203 verify
cargo run --release -p skillet -- test vm destroy clamps tpm-loss
cargo run --release -p skillet -- test vm destroy clamps header-recovery
```

The driver uses the vault cache. Clone copies only a stopped source disk and
creates independent TPM/NVRAM. Unlock sends the vault passphrase through an
unlogged console after automatic unlock fails. Rebind verifies the new TPM
before removing the old binding and saves a separate clone header. Verify checks
cold boot, volume identity, persistence, slots and the intact source unlock path.
Interrupted runs retain owned artifacts; archives survive disposal. Use
`test vm ready HOST SOURCE` to restore source readiness after this exercise.

See [VM operations](../../vm/README.md), [storage design](../../../../design/storage.md)
and the [remaining acceptance plan](../../../../plan/TPM-ENCRYPTED-ROOT.md).
