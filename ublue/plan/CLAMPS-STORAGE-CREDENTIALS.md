# Milestone 3 discussion: storage and credentials

Prerequisite: complete [the Skillet refactoring roadmap](SKILLET-REFACTOR.md)
before proceeding with the remaining feature milestones in this plan.
Existing acceptance evidence and deferred tasks remain applicable.

Status: remaining work, 2026-09-27. Base provisioning is implemented. The
storage mount path and subvolume layout are decided in
[the storage design](../design/storage.md); final physical disk remains open.
Credential decisions are recorded in [the secret design](../design/secrets.md).
Shared Butane storage setup, rootful Podman graphroot configuration, Skillet
mount guards and Pi-hole path changes passed fresh-VM, reboot, repeat-apply,
and mount-failure checks; see [the acceptance record](../butane/ACCEPTANCE.md).
The include is host independent and must be merged by each future host's
Butane template.

3B progress: exact KDBX lookup and safe use-or-create for a fresh host, an XDG
data-home vault default, a three-hour kernel unlock cache, encrypted SSH
delivery through the host Rust binary, a full-apply unit, and Pi-hole secret
file wiring are implemented.
Workspace checks, dummy credential delivery, repeated apply, rotation, reboot,
and VM smoke passed;
see [the acceptance record](../butane/ACCEPTANCE.md). The first smoke exposed
that the workstation denied expiry on keys
stored directly in the user keyring. Skillet now uses a named session keyring,
retained across CLI invocations through a link from the user keyring; its
permission and cross-invocation behavior were verified with a dummy key on
2026-09-28. Credential persistence passed an updated signed image deployment
and explicit digest-pinned rebase. Recovery after atomic credential install and
full-apply failure passed; a disconnect during streaming or encryption remains
untested. Production TPM assessment remains open. The Cloudflare creator uses
the implemented KDBX/account-token path; DDNS is a separate planned consumer.

## Split into two small steps

3A establishes the application-data mount. 3B delivers a disposable Pi-hole
password and proves recovery and rotation. Start with 3B on the existing VM;
settle storage before migrating production Pi-hole state.

## 3A: application storage

Decided: one Btrfs filesystem sharing space between OS and data, with a `data`
subvolume mounted at `/var/lib/data` and child `pihole` and `containers`
subvolumes. See [storage design](../design/storage.md). Final disk selection
remains open; the Samsung remains in rupik until cutover.

Read-only SSH inspection of boxy confirmed:

- `nvme0n1p2`, label `boxy`, supplies both `/os` mounted at `/` and `/data` mounted at `/var/lib/data`.
- System Podman uses driver `btrfs`, graphroot `/var/lib/data/containers/system`, runroot `/run/containers/storage`.
- Giacomo's rootless graphroot is `/var/lib/data/containers/giacomo`, also with driver `btrfs`.
- `sync` and `snapshots` exist under the data root. Listing all subvolumes requires privileges; passwordless sudo was unavailable, so the full subvolume inventory is not verified.
- Current mount flags include `noatime,lazytime,compress-force=zstd:1,discard=async,space_cache=v2,commit=120`. These are observed settings, not automatically selected defaults for clamps.

User-reported existing fleet differences: calculon mounts a separate Btrfs filesystem at `/var/lib/data`; rupik mounts one at `/srv`. Boxy's layout is the preferred model for the new fleet.

| Arrangement | Design consequence |
| --- | --- |
| Samsung for both OS and data | Requires a backup/restore or other explicitly designed repartitioning/install procedure; never assume the current data survives an OS installation |
| Stock SSD for both | Size the application data and headroom before committing; migrate data over the network or another agreed transfer route |

After that choice, establish whether the Samsung's existing filesystem/subvolume structure should be preserved or its data restored into a new layout. Use a read-only inventory of disk identifiers, filesystem types, sizes, subvolumes and used space when access is available. Do not infer these from Chef's `/dev/sda3` default.

Implementation decisions and checks:

- Keep uCore's composefs root and Btrfs `/sysroot` and `/var` layout. The
  existing VM uses `/dev/vda4` for the writable backing filesystem; prove the
  new `data` mount across reboot and rebase.
- Mount the `data` subvolume at `/var/lib/data`. Place Pi-hole under
  `/var/lib/data/pihole`, with its container `/etc/pihole` at `pihole/etc`.
- The shared Butane include sets system Podman's graphroot to
  `/var/lib/data/containers/system` on a fresh installation and keeps overlay.
  Verify SELinux labeling and the effective graphroot on a fresh VM. Existing
  populated stores require an explicit migration or fresh VM.
- Use stable disk/filesystem identifiers. Ignition handles installation-time
  formatting only for selected new/test storage; the shared Butane units create
  and mount `data`. Skillet verifies the mount and converges application
  directories, permissions and subvolumes without formatting populated disks.
- Make the data subvolume mount a prerequisite for Skillet, managed container
  services and auto-update. A failed data mount should leave SSH available
  where the OS can still boot, with dependent applications stopped and the
  failure visible. Manually invoked Podman is outside these unit dependencies.
  Loss of the shared physical disk also loses the OS; subvolumes do not provide
  device independence.
- Model the selected layout on one disposable VM disk. Test a failed/unavailable data subvolume mount rather than removing the shared OS disk.

Exit criteria for 3A:

1. Correct data filesystem mounted at the intended resolved path; ownership/modes survive reboot and repeat apply.
2. A sentinel file survives reboot and an OS rebase.
3. An unavailable or wrong data subvolume mount prevents dependent application startup and prevents writes into a fallback OS directory.
4. Restoring the correct subvolume mount permits recovery through normal convergence without formatting or data loss.
5. Repeated apply changes neither mount configuration nor directory metadata unnecessarily.

Fresh-VM graphroot, Pi-hole bind paths, SELinux label, reboot, repeated apply,
unavailable mount, wrong mount and recovery passed on `clamps-test-storage`.
The sentinel survived reboot and both mount-failure recoveries. Criterion 2
remains partial until an OS rebase preserves the sentinel. Final disk selection
and a stock-SSD install are also open.

Snapshots, Syncthing data transfer and final disk cutover are later milestones. Do not introduce all service subvolumes before their requirements are known.

## 3B: one credential end to end

Implement the [KeePassXC and delivery decision](../design/secrets.md):

- Read exact KDBX entries in workstation Rust code. Use a disposable test database
  to verify lookup, unlock failure and recovery on another workstation.
- Migrate the existing Cloudflare token creator from the desktop keyring to
  `skillet/cloudflare/token-creator` through KeePassXC; never print its value.
- Start delivery with the existing `pihole_web_password` credential. Production
  reads `skillet/hosts/clamps/pihole/web-password`; smoke uses a generated dummy
  value, never the production entry.
- Deliver through SSH stdin after base readiness. Encrypt on the guest and
  atomically install `/etc/credstore.encrypted/skillet/pihole_web_password.cred`.
  Wire the full-apply service's `LoadCredentialEncrypted=` and Podman consumer.
- Preserve secret bytes according to an explicit input format; do not trim
  meaningful whitespace implicitly. Validate encrypted output before replacement.
- Use host-key encryption for VM acceptance. Confirm production TPM support,
  rebase compatibility, and disk protection for Podman's separate secret copy.
- Rotation must recreate consumers; unchanged credentials must not restart them.
  A missing vault entry may create a production value only after the existing
  vault is unlocked and the host confirms neither an encrypted credential nor
  a Podman secret exists. A new workstation with a missing vault cannot create one.

Cloudflare per-VM issuance and cleanup follow in the live ACME milestone, using
the [VM lifecycle decision](../design/smoke-vms.md).

Exit criteria for 3B:

1. One documented provisioning path delivers the dummy credential automatically; a fixture verifies receipt without printing it.
2. Source control and generated unit/Quadlet text contain no plaintext value; command arguments and captured logs do not expose it.
3. Restarts, reboots and an OS rebase retain credential usability under the chosen policy.
4. Interrupted encryption/delivery/cleanup can be retried without losing the only usable credential.
5. A rotation reaches the running fixture; a subsequent apply causes no further recreation.
6. Missing, corrupt or undecryptable credentials produce a clear failure and follow a documented recovery procedure. They never fall back to an empty/default production password.

The first Pi-hole VM apply followed credential delivery. Storage acceptance is
required before production-state migration. Physical disk and production TPM
details still need resolution.
