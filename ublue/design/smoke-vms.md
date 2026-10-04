# Disposable smoke VM lifecycle

Decision: keep create, readiness, repeatable smoke checks, and destruction as
separate commands. Create includes a readiness barrier; the explicit ready
command supports retries. A retained VM supports inspection and repeated
convergence checks; a fresh VM establishes reproducible bootstrap. The fixture
tests real systemd/Podman convergence. Pi-hole credential delivery,
Tailscale enrollment/device cleanup, and opt-in Cloudflare/Caddy provisioning
are implemented. Live staging ACME/HTTPS acceptance passed on 2026-10-02;
production ACME and the deferred external-access checks remain unverified.

## Identity and ownership

The interface is `test-vm <host> <command> <instance>`; `list` may omit the host.
Host selects the Butane template and Skillet crate; instance identifies a run.
`clamps` and `smoke` become the internal domain `clamps-test-smoke`.
The ignored `butane/runs/<host>-test-<instance>/` directory records domain UUID,
disk, backend, artifact hash, SSH port, temporary SSH key, and known host key.
Commands validate ownership before acting, so an arbitrary libvirt domain name
does not authorize its deletion.

The libvirt runtime is shared across instances for a user/backend and recorded
in the manifest. Each concurrent VM has a distinct localhost SSH port. Native
libvirt and the existing virt-manager Flatpak support different workstation
OSes without installing host packages. The Flatpak runtime grants access to
the test artifacts and uses QEMU user networking.

## Rust lifecycle ownership

The lifecycle owner and migration order are recorded in
[the architecture decision](skillet-architecture.md) and
[the migration plan](../plan/REFACTOR-01-VM-LIFECYCLE.md). Public commands,
retained manifests, native/Flatpak support, and guest assertions are preserved.
`bin/test-vm` is a thin argument adapter. The separate `coreos-install`
lifecycle script and its hidden CLI bridge commands are retired. Rust selects
and prepares the recorded runtime, builds guest artifacts, resolves/downloads
the FCOS image, prepares and stages the run, invokes the standalone Butane
compiler, defines/starts the guest, and runs readiness. Updates build
both binaries, use verified recorded SSH trust, and save deployed hashes after
checking installed bytes and metadata. Original capture evidence is retained.
Disposal validates identity,
cleans external resources, then removes the UUID-addressed domain and local
artifacts. It also works with an already-absent guest; connection failures and
partial cleanup preserve journals for retry. Per-run locks protect individual
mutations, and a create-level lock serializes the full artifact, compilation,
and boot sequence. Readiness uses the selected profile's explicit
boot expectations and captured binaries, and records Ready only after signed
boot, unit, SELinux, resolver and user-environment checks. Bounded probes and
private diagnostics allow failed checks to be retried. It first persists
Started to invalidate stale readiness, then checks ownership before and after
each guest command/upload. Creation persists a private `Preparing` intent and
UUID first. Rust atomically prepares the disk and SSH key, stages host templates,
includes and binaries, applies VM-only Ignition changes, and records captured
hashes and compatibility metadata. The standalone Butane compiler still builds
Ignition. The generated UUID is part of native XML and Flatpak creation. Native
definition and start are separate manifest phases; failed starts retain the
defined domain for inspection and retry. Native domain XML renders through the
Rust XML library, preserving disk ownership, fw_cfg and passt networking. Unit
and adapter checks pass; live migration acceptance remains deferred.

Tailscale enrollment intent, enrolled-device identity, and Cloudflare DNS/token
ownership metadata are persisted by `skillet_workstation::provisioning_state`.
Writes are atomic and mode `0600`; readers reject symlinks and non-regular
files. `skillet_workstation::tailscale_enrollment` owns enrollment sequencing:
it persists intent before provider or guest mutations, treats a failed guest
probe as an error, and removes intent only after the tagged device identity is
recorded. An existing pending marker must match the same VM identity, so retry
cannot silently claim another VM's cleanup journal.
New journals keep host, environment, and instance as separate fields. Legacy
smoke-run records remain readable and are checked against the exact expected
guest hostname, ownership marker, and token name rather than parsed into a new
identity.

## States and recovery

Planned encrypted-root tests add a software TPM2 and UEFI/Secure Boot profile.
Each VM owns independent TPM state and firmware NVRAM alongside its disk;
reboots preserve them and disposal removes only owned state. Software TPM
checks establish provisioning behavior; physical hardware acceptance remains
required. See the [TPM encrypted-root plan](../plan/TPM-ENCRYPTED-ROOT.md).

1. **Create:** build the generic CLI and host binary, allocate the run directory,
   and persist a `Preparing` manifest before local artifact changes. The Rust VM
   library atomically copies the selected image, records its digest, and creates
   or validates the run's Ed25519 SSH key pair. It stages and specializes the
   host Butane source, shared includes, and binaries, and records their hashes.
   The standalone Butane compiler builds Ignition from that staged tree.
   Ignition contains the public SSH key and base configuration. Large binaries travel over SSH because embedding them
   in QEMU `fw_cfg` previously caused long boot delays.
2. **Ready:** wait for SSH, deliver both binaries, complete unsigned then signed
   rebase, and check the booted deployment, base apply, resolver, SELinux, and
   user environment. Persistent bootstrap state tolerates reboots. Executable
   staging uses `/var/usrlocal/bin`, whose FCOS labeling permits execution.
   Ignition's authorized-key file remains configured alongside dotfiles keys.
   `/etc/skillet/host` keeps host profile identity independent from a VM's
   hostname. `ready` uses the artifacts captured at creation. Explicit `update`
   builds and installs current shared and host code on a retained VM, recording deployed hashes
   separately while preserving the original creation snapshot.
3. **Provision applications:** `test vm provision` unlocks the workstation
   KeePassXC database and supplies the Tailscale OAuth client, test policy,
   VM identity, and verified guest transport to
   `skillet_workstation::tailscale_enrollment`. That workflow reuses an existing
   enrollment or mints and delivers a one-use smoke-tagged key through the
   [shared secret mechanism](secrets.md), waits for the VM to join, and records
   its device identity. The CLI then delivers a generated Pi-hole credential
   and runs full apply. Repeated provisioning
   reuses the Tailscale identity and installed Pi-hole credential; `--rotate`
   replaces the latter. `--with-ui` also creates a 12-hour zone-scoped
   Cloudflare token, reconciles tailnet A/AAAA and UI/alias CNAME records,
   installs both Caddy credentials, and activates the Caddy apply unit. The
   ignored mode-0600 Cloudflare metadata records its token ID, expiry, marker,
   and DNS record IDs, never the token value.
4. **Smoke:** run assertions against the retained VM. Check repeat apply,
   configuration/secret changes, interruption recovery, and reboot persistence.
   The container sandbox uses mocked systemctl/Podman; only VM checks establish
   real runtime behavior. Failures retain the VM and diagnostics for repair.
5. **Destroy:** validate ownership, mint a short-lived scoped cleanup token,
   remove only DNS records carrying this instance's ownership marker, revoke
   its disposable tokens, remove the recorded smoke-tagged Tailscale device,
   then remove the domain, disk, SSH key, and artifacts. Failed external
   cleanup prevents local destruction and retains metadata for retry, even if
   the domain is already gone.

## Live ACME boundary

The [Cloudflare ownership lifecycle](cloudflare-ui-lifecycle.md) is implemented
for token issuance and DNS cleanup after expiry. Live ACME acceptance remains
pending; do not require a manually created VM token.

Routine smoke checks need no Cloudflare access. The optional live scenario
enrolls the VM in Tailscale, obtains its tailnet IP, and creates a DNS-only
machine A/AAAA record set and UI/alias CNAMEs under the existing smoke-test
Cloudflare zone using a per-VM
token minted by the workstation token creator. It uses Let's Encrypt staging
and a unique hostname, preserving the zone's other records. Keep the literal
zone outside this public repository. DNS-01 needs no publicly reachable VM.
Staging certificates are untrusted; the test verifies their expected identity
using explicit staging trust. Test name resolution through a client's actual
resolver because some resolvers block public answers containing tailnet IPs.
Normal reruns reuse credentials and certificate state; fresh issuance is an
explicit scenario. Production DNS cutover is separate from VM acceptance.

Acceptance requires a fresh create, readiness, application checks, unchanged
second apply, and reboot persistence. Destruction must also demonstrate that
external credentials and test-owned resources are cleaned up. An interrupted
run must either resume or leave enough ownership metadata for cleanup.
