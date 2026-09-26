# Milestone 3 discussion: storage and credentials

Status: draft, 2026-09-25. Parent-owned planning document while agents implement milestones 1 and 2. User prefers boxy's single Btrfs filesystem with separate OS/data subvolumes. Mount path, final disk and credential policy remain open.

## Split into two small steps

3A establishes a persistent application-data mount and proves it behaves correctly. 3B delivers one dummy Pi-hole password end to end and proves recovery and rotation. Pi-hole itself is introduced in milestone 4.

## 3A: application storage

Agreed direction: one Btrfs filesystem sharing space between OS and data, with separate subvolumes, following boxy's approach. The particular data mount path is not important to the user. Final disk selection remains open; the Samsung remains in rupik until cutover.

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

Proposed defaults for discussion:

- Keep OS/data subvolumes on the shared filesystem while respecting uCore's deployment layout. Do not assume boxy's ordinary `/os` root mount can be copied literally into an ostree installation; prove the installation/rebase mechanism in the VM.
- Proposed canonical data mount: `/var/lib/data`, matching boxy and calculon. An alternative remains `/srv`; settle the path before implementation.
- Place Pi-hole under `<data-root>/pihole`, preserving its configuration layout during migration rather than requiring its old absolute host path.
- Proposed system Podman graphroot: `<data-root>/containers/system`, following boxy. Confirm the storage driver and SELinux labeling on uCore before committing it. Configure before the first application container; never switch the driver of an existing populated store implicitly. Add rootless storage only if clamps actually needs it.
- Use stable disk/filesystem identifiers. Ignition handles installation-time formatting only for explicitly selected new/test storage. Skillet converges mounts, directories, permissions and eventual subvolumes without formatting populated disks.
- Make the data subvolume mount a prerequisite for directory creation, container storage use and application startup. A failed data mount should leave SSH available where the OS can still boot, with dependent applications stopped and the failure visible. Loss of the shared physical disk also loses the OS; subvolumes do not provide device independence.
- Model the selected layout on one disposable VM disk. Test a failed/unavailable data subvolume mount rather than removing the shared OS disk.

Exit criteria for 3A:

1. Correct data filesystem mounted at the intended resolved path; ownership/modes survive reboot and repeat apply.
2. A sentinel file survives reboot and an OS rebase.
3. An unavailable or wrong data subvolume mount prevents dependent application startup and prevents writes into a fallback OS directory.
4. Restoring the correct subvolume mount permits recovery through normal convergence without formatting or data loss.
5. Repeated apply changes neither mount configuration nor directory metadata unnecessarily.

Snapshots, Syncthing data transfer and final disk cutover are later milestones. Do not introduce all service subvolumes before their requirements are known.

## 3B: one credential end to end

Retain the intended SOPS/age -> bootstrap delivery -> encrypted systemd credential -> Skillet -> Podman -> container sequence, with explicit decisions at its boundaries.

Decisions to take after storage:

1. Is a restricted, temporary plaintext-bearing Ignition artifact acceptable during provisioning, or must plaintext never be persisted on the workstation? This changes the transport/build workflow.
2. Must production credentials require the TPM? How will reinstall, TPM failure and recovery work? VM testing must use a documented test policy or vTPM and must not silently weaken the production policy.
3. Is Podman's protected default file store sufficient with the chosen disk protection, or is encrypted secret storage required there too? Systemd credential encryption alone does not protect a second stored copy.

Proposed implementation size:

- Start with the existing named `pihole_web_password` credential; defer a general JSON secret bundle unless multiple credentials justify it.
- Use a dummy value in VM acceptance. Add only one encrypted source entry, one safe transport path, one bootstrap encryption service and one consuming fixture.
- Preserve secret bytes according to an explicit input format; do not trim meaningful whitespace implicitly.
- Encrypt to a temporary output, validate it, then atomically install it. Make cleanup/retry safe across interruption at every stage. Deleting a temporary file is cleanup, not a claim of guaranteed physical erasure.
- Keep the encrypted source recoverable independently of the target TPM. Document the age-key backup/recovery process without storing private keys in the repository.
- Rotation must deliver the replacement credential and recreate its consumers. Repeated application of an unchanged credential must not restart them.

Exit criteria for 3B:

1. One documented provisioning path delivers the dummy credential automatically; a fixture verifies receipt without printing it.
2. Source control and generated unit/Quadlet text contain no plaintext value; command arguments and captured logs do not expose it.
3. Restarts, reboots and an OS rebase retain credential usability under the chosen policy.
4. Interrupted encryption/delivery/cleanup can be retried without losing the only usable credential.
5. A rotation reaches the running fixture; a subsequent apply causes no further recreation.
6. Missing, corrupt or undecryptable credentials produce a clear failure and follow a documented recovery procedure. They never fall back to an empty/default production password.

Advance to real Pi-hole only after both steps pass. The shared-filesystem/subvolume direction is selected; its remaining details and credential choices remain pending discussion.
