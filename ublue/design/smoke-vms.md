# Disposable smoke VM lifecycle

Keep create/readiness, repeatable smoke and destruction separate. Creation has
a readiness barrier; retained VMs permit inspection/retry, while fresh VMs prove
bootstrap. One Rust owner implements lifecycle policy for native and Flatpak
adapters; shell wrappers only adapt arguments. [Operating guide](../skillet/crates/vm/README.md).

## Identity and ownership

Keep host profile, environment, instance, runtime hostname and SSH target separate.
The manifest records resolved identity, UUID, disk/firmware paths, runtime URI,
SSH trust and artifact hashes. Never reconstruct ownership from composite names.
Per-run locks serialize operations; each guest call revalidates recorded ownership
before and after. Lost ownership blocks further contact.

Create intent and UUID precede artifacts or domain changes. Atomic writes retain
recovery after partial failures. Status reads do not migrate legacy state;
explicit import validates and preserves original files. A failed runtime query
is not evidence that a domain is absent.

## Rust lifecycle ownership

The owner selects an existing runtime, builds exact Cargo-reported artifacts,
prepares disk/key/staged Butane and invokes the standalone compiler. Native XML
and Flatpak adapters consume the same declared identities. Large binaries travel
over SSH after first boot rather than QEMU fw_cfg. Captured hashes remain distinct
from later verified update hashes.

Ready validates signed **booted** image, base unit, installed bytes/metadata,
resolver/SELinux/masked-unit policy and user environment. It invalidates stale
Ready first; only actual evidence restores it. Bounded probes and private failure
logs preserve the original error. Update builds current artifacts; ready uses
the creation capture. Guest assertion helpers do not own SSH/reboot/retry.

## States and recovery

- Preparing intent → compiled/staged artifacts → defined domain → started guest
  → verified Ready. Each transition persists recoverable state.
- Provisioning stages the complete credential set before its consumer, reuses
  disposable identity, and journals provider ownership before mutations.
- Smoke persists a non-ready checkpoint before reboot and proves a changed boot
  plus runtime readiness before restoring Ready. Interrupted recovery resumes
  through the same owner in a later process.
- Destruction validates ownership, completes external cleanup, removes the
  UUID-addressed domain and owned local artifacts, then removes recovery metadata.
  Partial/already-gone resources are handled without treating failures as absence.

## Live ACME boundary

Routine smoke uses fixtures. Opt-in provider checks select test configuration,
scoped bounded tokens and staging ACME. Unique ownership does not silently alter
UI hostnames; namespace collisions fail. The [Cloudflare owner](cloudflare-ui-lifecycle.md)
journals records/tokens and cleans them before the Tailscale device/local VM.
Expiry is not cleanup, and failed cleanup retains metadata.

Identity-protected Pi-hole needs no password; retained native-password callers
use disposable credentials without production entries. Hosted Tailscale
OAuth mints one-use tagged keys; journals bind enrolled device IDs/addresses to
the complete test identity. Provider secrets stay off manifests and diagnostics.
Fixture success, application success, separate-network denial and production
certificate acceptance are distinct evidence scopes.

## Local UI inspection

Owned SSH forwards bind both ends to localhost for the interactive session.
Duplicate/busy ports fail. Forwarding does not fence restored controllers;
production-state import needs a persistent isolation policy verified while the
guest is empty, with rollback timers cancelled before import.

## Optional encrypted root

Root profile is independent of host identity; retained manifests without it mean
unencrypted. TPM profiles require UEFI/Secure Boot/TPM2 and fail rather than
silently downgrade. Independent runs own independent TPM state/NVRAM, preserved
across cold starts and removed only by owned disposal.

Encryption acceptance uses private disposable recovery artifacts; instance-scoped
vault recovery archives are a separate workflow and survive disposal. TPM-loss
acceptance clones only a stopped base-only disk, records ancestry, uses new
TPM/NVRAM and an unlogged recovery console, and preserves the source unlock path.
A compromised hypervisor is outside software-TPM protection claims.

Unfinished acceptance: [refactor](../plan/SKILLET-REFACTOR.md),
[private UIs](../plan/GENERIC-PRIVATE-UIS.md), [TPM](../plan/TPM-ENCRYPTED-ROOT.md).

User-environment readiness checks installed Brewfile dependencies with
`--no-upgrade`. Package freshness belongs to the update lifecycle; a newly
available formula revision must not invalidate an already satisfied prerequisite.
