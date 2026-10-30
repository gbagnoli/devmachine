# Historical design progress

Documentation snapshot, 2026-10-13. These are previous status/progress passages,
not new acceptance evidence. Some old statements are superseded; use current
[active plans](../README.md), designs and operating guides for current behavior.


## skillet-architecture.md

Decision, 2026-10-03: complete the seven refactoring workstreams before further
service migrations, encryption implementation, or production cutover. Existing
service behavior and unfinished acceptance requirements remain authoritative.
Status: follow-up implementation and scripted interrupted-reboot recovery are
complete; remote CI and a Syncthing data ownership divergence found during
repeat application acceptance remain open. Rust-owned fixture smoke, retained
VM update, and the earlier capability-selected application acceptance passed
locally; see the
[follow-up plan](REFACTOR-FOLLOWUP.md) and
[acceptance log](ACCEPTANCE.md). Production acceptance remains
explicitly deferred.

Implementation progress: `skillet_vm` now supplies validated run identity and
versioned manifests with explicit, non-destructive legacy import. Rust status
and disposal use a focused libvirt adapter and a locked cleanup orchestrator;
listing is also in Rust. Shared Cargo JSON artifact discovery selects the exact
reported binary and now serves the container runner and retained VM updates.
Updates share the ownership lock and SSH transport, preserve captured hashes,
and record deployed hashes only after verifying both installed artifacts.
Readiness now uses the same locked owner and SSH transport, with explicit
boot expectations from the canonical host profile, bounded probes and private
diagnostics. It verifies original artifacts and signed boot evidence before
recording readiness. A retry first clears the persisted Ready checkpoint and
restores it only after acceptance. Guest operations are guarded by ownership
checks before and after each call. Production and disposable-VM credential
delivery use the same verified guest transport and typed credential installer.
Workstation modules now own host credential delivery, UI/DNS derivation,
provider mutations, and recovery cleanup; the CLI opens the vault, loads the
canonical VM manifest, and dispatches typed operations. Disposable provisioning
holds the VM run lock and revalidates recorded backend ownership around each
guest operation through a shared checked transport. Transport's bounded I/O
stays in memory; payload copies are zeroized after stdin transfer. Earlier
fixture and provisioning acceptance passed. Rust now owns smoke reboot and
capability-selected application acceptance. The latter has passing injected
tests and a named-VM repeat-apply/reboot acceptance. Production acceptance
remains deferred.

Workstream 2 adds `skillet_hosts` as the single host capability declaration.
It owns host composition and supplies boot, service, network, UI, storage, and
credential policy to entry points. The explicit `agent` baseline is the generic
no-service apply profile. Synthetic smoke fixture composition lives in a
separate test command and is not dispatched as a host identity. Workstation
Tailscale enrollment now accepts provider, profile/environment, VM identity,
state path, and guest transport as explicit inputs. Retained-VM
runtime acceptance remains pending.

Workstream 3 implementation is complete. Account observations and
subordinate-ID files enter composition through injected resource interfaces;
missing or invalid subordinate ranges fail closed. File and directory
mutations share the typed named-or-numeric `Ownership` contract, including
numeric IDs that do not require a matching host account. Entry points pass
required systemd credentials to host composition. Host composition retains
aggregate effect interfaces during migration. The completion audit added
stricter Btrfs and service-state behavior to the fakes; those contract
corrections are tracked by the [follow-up plan](REFACTOR-FOLLOWUP.md).

## secrets.md

UI provisioning reads a mandatory Cloudflare Zone ID and optional relative
UI prefix from the selected environment in KeePassXC, fetches the zone domain,
and derives service names from the host's declarations,
and uses shared credential delivery. Cloudflare token issuance, DNS lifecycle,
and smoke VM cleanup are implemented. Staging ACME and HTTPS acceptance passed
on 2026-10-02; production renewal remains pending.
See the [generic UI plan](../GENERIC-PRIVATE-UIS.md).

Planned disk recovery: production LUKS recovery keys use
`skillet/hosts/<host>/storage/root-recovery-key` and remain available off the
protected host. Root unlock uses the TPM, independently of vault access or
Skillet application-credential delivery. Disposable encryption tests generate
their own recovery material. Header backups require protected off-machine
storage; implementation and recovery checks are in the
[TPM encrypted-root plan](../TPM-ENCRYPTED-ROOT.md).

Secret auditing uses shared public requirement metadata to render the root
checklist and supply missing-entry instructions. Required manual entries follow
the selected canonical host capabilities and environment; template references
come from the delivery resolver, including host overrides. Generated, optional,
and planned entries are distinguished. A read-only inventory of entry paths
finds unused Skillet entries across all declared hosts and both environments,
retaining known migration paths. It never emits values, mutates the vault, or
uses provider APIs; it checks declared field presence/formats rather than provider
permissions or host recovery state. Existing entries default to Password-only
validation. Structured entries declare standard/custom fields and validators in
the shared metadata. `skillet/smtp` uses Username/Password and host/port/tls
and sender custom fields. Its shared fleet audit requires the entry for production
only. SMTP consumption uses a dedicated native apply phase; disposable
machines receive a credential-free capture payload without consulting the entry. `secrets` is the canonical CLI group; `secret` remains an alias.

Optional DDNS uses the same encrypted delivery path and a separate apply unit.
The workstation renders the shared configuration template, resolves its typed
KeePassXC references, and combines it in memory with a zone-scoped child token.
The guest mounts the Podman secret as
`/config.json`, root-only. The master token is never delivered. Production
delivery and journaled disposable ownership/cleanup are implemented; live
disposable DNS acceptance awaits its test name entry. See
[DDNS design](../../design/cloudflare-ddns.md).

The [Cloudflare lifecycle decision](../../design/cloudflare-ui-lifecycle.md) is implemented
for credential issuance, DNS reconciliation, and disposable cleanup. No manual
smoke VM ACME token is required.

Smoke VMs get generated test passwords, never production host entries. Tailscale
enrollment reads `skillet/tailscale/provisioner-client-id` and
`skillet/tailscale/provisioner-client-secret` locally, then mints a one-use,
preauthorized, non-ephemeral key tagged for that smoke VM. The key expires in
one hour if unused and is delivered through the same SSH, systemd, and Podman
secret path as other credentials. The workstation verifies the VM's tailnet
addresses and saves its device ID and addresses in mode-0600 run metadata. On
destroy, Skillet uses `devices:core` to find and remove only the device with the
recorded name, smoke tag, and identity. If enrollment or deletion is
interrupted, pending metadata remains so cleanup can be retried. Production
uses the server tag and the same one-use-key path.

Opt-in `test vm provision <host> <instance> --with-ui` reads
`skillet/cloudflare/token-creator` locally and mints a 12-hour Cloudflare token
per disposable instance, restricted to the selected zone with Zone Read and DNS
Write. After Tailscale enrollment, it reconciles machine A/AAAA records for the
verified tailnet addresses, canonical UI CNAMEs, and declared alias CNAMEs.
The token supports Caddy DNS-01. DNS Write scopes access to the whole zone;
unique test names prevent record collisions, not access to other records. On
destroy, Skillet issues a short-lived cleanup token, removes only records with
the instance ownership marker, and revokes disposable tokens. Keep literal
zone names and records outside this public repository. The token creator
remains on the workstation. Staging ACME and HTTPS acceptance passed on
2026-10-02; production renewal and external access remain pending.

Root recovery uses an independent printable high-entropy passphrase in Password,
with protected `luks-uuid` and `recovery-state` metadata. Save a pending entry
atomically before enrollment; verify against the same volume before marking it
verified. Retries reuse pending keys; verified keys are never silently re-enrolled
or replaced. Production paths are `skillet/hosts/<host>/storage/root-recovery-key`;
VM archives use `skillet/environments/test/hosts/<host>/instances/<instance>/storage/root-recovery-key`.
VM disposal retains vault archives deliberately, so new disks require an explicit
archive removal or a fresh instance name. Keys travel only through verified SSH
stdin and a restricted guest `/run` file removed after use. This workflow requires
provisioning-time noninteractive sudo and precedes access finalization.

## smoke-vms.md

Decision: keep create, readiness, repeatable smoke checks, and destruction as
separate commands. Create includes a readiness barrier; the explicit ready
command supports retries. A retained VM supports inspection and repeated
convergence checks; a fresh VM establishes reproducible bootstrap. The fixture
tests real systemd/Podman convergence. Pi-hole credential delivery,
Tailscale enrollment/device cleanup, and opt-in Cloudflare/Caddy provisioning
are implemented. Live staging ACME/HTTPS acceptance passed on 2026-10-02;
production ACME and the deferred external-access checks remain unverified.

The lifecycle owner and migration order are recorded in
[the architecture decision](../../design/skillet-architecture.md) and
[the migration plan](REFACTOR-01-VM-LIFECYCLE.md). Public commands,
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
and adapter checks pass. Rust now owns fixture smoke delivery, guarded guest
operations, reboot detection, and readiness recovery. The named fixture smoke,
retained update, and capability-selected application repeat-apply/reboot
acceptance passed on 2026-10-05. Interrupted-reboot fault injection remains
pending in the [follow-up plan](REFACTOR-FOLLOWUP.md) and
[acceptance log](ACCEPTANCE.md).

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

Planned encrypted-root tests add a software TPM2 and UEFI/Secure Boot profile.
Each VM owns independent TPM state and firmware NVRAM alongside its disk;
reboots preserve them and disposal removes only owned state. Software TPM
checks establish provisioning behavior; physical hardware acceptance remains
required. See the [TPM encrypted-root plan](../TPM-ENCRYPTED-ROOT.md).

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
   [shared secret mechanism](../../design/secrets.md), waits for the VM to join, and records
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

The [Cloudflare ownership lifecycle](../../design/cloudflare-ui-lifecycle.md) is implemented
for token issuance and DNS cleanup after expiry. Live ACME acceptance remains
pending; do not require a manually created VM token.

Implemented: `test vm ssh --forward LOCAL:GUEST` exposes a guest localhost
TCP endpoint only on workstation localhost. The shared SSH adapter owns the
forward for the interactive session; the existing lifecycle holds its run lock
and checks domain ownership before and after that session. Bind failures are
fatal and duplicate local ports are refused. This allows browser inspection
without publishing a guest application port on LAN or tailnet. A forward is
not a network isolation policy; restored production controllers still require
a persistent guest fence before import.

Each manifest records a root profile; absent fields in retained manifests mean
`unencrypted`. Explicit `tpm` requires UEFI Secure Boot with enrolled keys and
an emulated TPM2 SHA256 bank. Native and Flatpak definitions share the same
XML policy for this profile; Flatpak live equivalence remains unverified.
TPM state, NVRAM, serial diagnostics and recovery artifacts belong to the named
run. Lifecycle ownership verifies the firmware/TPM paths in addition to UUID
and disk. Cold starts preserve them; disposal removes them. Independent runs
never share TPM or firmware state.

## storage.md

Decision, 2026-10-02: add LUKS2 beneath the shared Btrfs backing filesystem
and prefer local TPM2 unlock on both local and remote physical hosts. Normal
boots must not need a network or operator. This protects persistent service
state, including Podman's disk copies of secrets, while retaining the existing
subvolume layout. Tang is deferred; assess each remote host's TPM first.
EFI and `/boot` remain outside encryption. An explicit boot-integrity policy
and update/rollback compatibility must be proved before claiming protection
against theft of the whole machine. Keep an independent recovery key in the
private vault and a protected off-machine LUKS header backup.
Decision, 2026-10-10: the first profile uses native Ignition/Clevis TPM2 with
SHA256 PCR 7 and requires Secure Boot. Its accepted protection scope is removal
or loss of the encrypted disk; separate GRUB kernel/initramfs files do not
provide authenticated whole-machine boot. Custom UKIs and signed PCR policies
are deferred. No network service participates in unlock.

Implemented for disposable installs: `unencrypted` remains the default;
explicit `tpm` selects LUKS2 beneath Btrfs and fails when prerequisites are
missing. There is no automatic downgrade. Profile selection is independent of
host identity. Filesystem UUID/label checks support both layouts. TPM enrollment
occurs during native Ignition installation; the same firmware policy must
survive the signed-uCore bootstrap. Changing Secure Boot/MOK policy requires a
working recovery path before enrollment changes. On physical hardware, enroll
any required image signing/MOK keys before the first encrypted Ignition boot;
QEMU success with its enrolled firmware keys does not replace that procedure.

Disposable acceptance creates an independent random recovery key and header
backup under the private run directory, then checks recovery without TPM and
cold-boots through the lifecycle owner. Only TPM and recovery keyslots are
accepted. These artifacts are deleted on disposal. Production vault enrollment
is implemented with write-ahead persistence and volume identity checks.
Live VM vault enrollment/retry and cold-boot persistence passed on 2026-10-10;
invalid-passphrase rejection confirms the independent passphrase path.
Header export is implemented: the recovery-key command archives a verified
LUKS2 header as the protected `root-luks.header` attachment on the same KDBX
entry. It refreshes after enrollment changes without rotating the passphrase.
The encrypted vault, backed up independently, carries both recovery artifacts.
A header backup is not a data backup. Physical-disk selection and physical
recovery acceptance remain planned. Skillet convergence never formats an installed filesystem.
QEMU functional tests and physical acceptance are specified in the
[TPM encrypted-root plan](../TPM-ENCRYPTED-ROOT.md).

The clamps VM passed fresh provisioning, mount, SELinux, reboot, repeat apply,
and unavailable/wrong-mount recovery. A post-provision OS rebase and the
physical disk choice remain open in
[the storage plan](CLAMPS-STORAGE-CREDENTIALS.md).


## TPM capability audit (2026-10-10)

Current boot entries use separate kernel/initramfs files under GRUB. Neither
ukify nor systemd-measure was found on the deployed guest; no signed PCR-policy
pipeline exists in our image recipes. PCR 7 binds Secure Boot policy, not all
kernel/initramfs/command-line bytes. TPM-only unlock must not be described as
protection against whole-machine theft or tampering. Signed-policy boot integrity
requires a separate implemented update/rollback chain; adding packages alone
would not establish it. The operator accepted the narrower native profile above; stronger boot integrity
is a separate future project. Virtual acceptance does not establish physical
firmware compatibility.

Recovery acceptance uses a stopped, base-only disk clone with independent TPM
and NVRAM. It preserves the source TPM and archive; provider-provisioned VMs
are excluded to prevent impersonation. Recovery consoles have no serial log.
New TPM enrollment is explicit, verifies the new binding before removing the
old one, and archives the refreshed clone header separately. Routine convergence
never replaces TPM bindings. Native header restoration, TPM-loss recovery/re-enrollment, completed retry and
unattended cold boot passed on 2026-10-11; evidence is in Butane's record.

Physical installation uses an explicit destination whole-disk by-id path and
reuses the VM-tested root-profile transform. Workstation media generation never
opens a block device. A separate live helper checks the actual server and asks
for disk confirmation before installation; TPM media requires the final Secure
Boot/MOK policy before first encrypted boot. VM staging retains `/dev/vda`.
The live ISO contains a small bootstrap configuration, operator public key and
expected destination-Ignition digest. CoreOS's 256 KiB embedding limit excludes
our application binaries, so the full destination payload is transferred over
verified SSH (or separate data media), then checked in root-only live tmpfs.
Production media requires private bootstrap-access inputs instead of inheriting
committed keys/password hashes. USB media generation is implemented; physical
boot and installation acceptance remain unverified. See the
[hardware runbook](../../butane/HARDWARE.md).


## monitoring.md

Decision, 2026-10-06: services own their monitoring declarations. Datadog is an
optional host capability; the agent consumes the checks belonging to the
services selected by the canonical host caller. Adding or removing a service
must also add or remove its monitoring without a second service inventory.


Approved agent deployment, 2026-10-07: a pinned official rootful Agent container managed by
Skillet. Datadog recommends container deployment for Podman over native host
installation, which requires additional permission handling. The OS image
continues to provide Podman and kernel tooling; Skillet owns activation,
credentials, and changing integration configuration. Packaged community
integrations with extra dependencies require a reviewed image build; never
install them through runtime downloads.

Decision, 2026-10-12: native service collectors may instead use the existing
static musl guest binary and submit through DogStatsD. The official Agent stays
unchanged. DogStatsD uses a Unix datagram socket under a root-only `/run`
directory, recreated by tmpfiles on boot; its UDP listener is disabled. The
Agent owns the socket and ingestion credentials. Collectors receive neither
API keys nor vault access. Module-owned timers are configured only after the
credential-gated Agent phase succeeds, and must not activate that Agent or an
optional application themselves.

Implemented agent runtime and credential delivery. Host monitoring
needs host PID/cgroup visibility, read-only proc/cgroup/filesystem mounts, and
SELinux access. Podman Autodiscovery through the rootful API socket grants
container-management authority even with a read-only socket bind mount.
Preserving Chef's network/service monitoring additionally needs the
system-probe's eBPF capabilities and debugfs access. Those privileges must be
explicit caller policy and tested on uCore with its Secure Boot/SELinux settings.
For network monitoring, explicitly select `PERFMON`: the installed Podman
seccomp profile gates `perf_event_open` on that capability. `SYS_ADMIN` alone
left the syscall returning ENOSYS in the VM. Preserve default seccomp and
read-only debugfs; do not disable syscall filtering or make tracing writable
unless a separate diagnosed requirement warrants it. Named-VM acceptance on 2026-10-08 confirmed perf events, NPM/USM traffic
collection and automatic recovery after reboot with SELinux enforcing. The VM
uses BIOS; Secure Boot remains a physical/UEFI acceptance requirement.

Host networking preserves host interface visibility and localhost SSH checks;
Autodiscovery reaches container bridge addresses without published UI ports.
The socket is accessed through a read-only bind of `/run/podman`; the volume
helper converges directory roots, not socket files. Integration directories
are mounted individually so the official image retains its default system and
container checks. The pinned official image
requires `DD_API_KEY` at startup, so Podman injects
API key and private tags through native environment secrets; the public site
uses an ordinary `Environment=DD_SITE` directive. No literal secret-bearing
`Environment=` directives are written. Production and test use the shared key;
delivery adds an authoritative environment tag and region. Production region
is declared by the host caller (Clamps inherits Chef's `region:ftwo`); every
test deployment uses `region:lab`, replacing any template region tags.
Monitor notification policy excludes `env:test` in the data query, preserving
legacy production hosts with `env:home` or no environment tag. Monitor metadata
tags alone do not filter host data. One-time monitor administration uses a
separate optional workstation-only application key; it is never delivered to
the Agent. One-time setup completed on 2026-10-08: the all-host Agent no-data and NTP
monitors exclude `env:test`. Provider validation, updates and query read-back
passed; notification settings were preserved. Other existing monitors target
production hosts explicitly. New monitors must preserve this exclusion.
Agent status and successful submissions confirm the delivered test/lab tags;
provider host-tag read-back returned HTTP 403 with the monitor-only key. Both
monitors returned zero test-host groups before and after reboot. PagerDuty
incident delivery was not separately observed. Test
telemetry requires explicit `--with-datadog`; destroying a VM does not remove
historical telemetry from Datadog.

Implemented: labels for Syncthing HTTP health and process liveness for
Pi-hole, DDNS, UniFi, Caddy, and Tailscale; Agent deployment/delivery and
host/process/network/Btrfs/SSH/systemd configuration. Named-VM acceptance
confirmed host/container checks, Autodiscovery, and successful submissions with
SELinux enforcing. eBPF acceptance now passes with explicit `PERFMON`, default
seccomp and read-only debugfs. Real Syncthing bridge traffic was classified as
HTTP with nonzero byte counters and HTTP aggregates, also after reboot. Pending:
private ping targets and richer
Syncthing metrics and Pi-hole's authenticated API integration. The existing
Pi-hole community integration requires compatibility review against Pi-hole 6.

## private-ui-access.md

Use host-specific names in existing Cloudflare-managed production and smoke-test
zones rather than `*.ts.net`, so UI URLs and Caddy routes can survive a move
between hosted Tailscale and Headscale. Publish DNS-only A/AAAA records for
`<host>.<ui-domain>` pointing to its tailnet addresses. Canonical UI names CNAME to that machine name;
caller-declared aliases CNAME to their canonical UI names. Caddy serves every
name with matching certificate coverage and the same upstream/access policy.
The machine/CNAME/alias model is implemented; see the
[Cloudflare lifecycle](../../design/cloudflare-ui-lifecycle.md). Do not enable Cloudflare
proxying. Keep literal
zones and records outside this public repository; public DNS still exposes the
names and addresses. Some resolvers block public answers containing tailnet
addresses as DNS rebinding, so check resolution from real clients before
cutover. Caddy gets publicly trusted certificates through Cloudflare DNS-01;
the UIs need no public route. Cloudflare's certificate is not used for DNS-only
records: Caddy serves the certificate.

Restrict Caddy's UI listener to tailnet traffic and grant access only to the
intended user identities. Tailnet access authorizes the connection; it does not
create an application login. Keep application passwords for existing services until the private path
and absence of direct UI access have been verified, then decide per application
whether a second login is useful. File browsing and Gokapi administration now
have an explicit no-second-login requirement: the shared
[tailnet identity gate](../../design/tailnet-ui-authentication.md) authenticates and authorizes
the two intended users before proxying. It is implemented for Mikochi; Gokapi
integration remains planned. An IP-range check alone is insufficient
for those services. In particular, Syncthing's current wildcard
host publication of GUI port 8384 must be removed or limited to a safe local
proxy path before relying on tailnet access alone.

The [Cloudflare ownership lifecycle](../../design/cloudflare-ui-lifecycle.md) now issues
credentials automatically and cleans owned DNS even when a disposable token has
expired. Staging ACME acceptance passed; no manually created VM token is required.

Host UI service declarations now drive a shared versioned Caddy payload.
Provisioning derives `<service>.<host>.<ui-domain>` from the selected
environment's resolved namespace: the optional KeePassXC `dns/ui-domain`
relative prefix (default `ui`) appended to the zone domain fetched using the
mandatory `dns/cloudflare-zone-id`. Caddy validates the payload against the host's
declaration and renders each route through bridge DNS with tailnet source
restrictions. Shared Butane units call the generic `skillet` CLI and read the
stable host profile from `/etc/skillet/host`; VM hostnames remain independent.
Host-specific apply units and credential gates live in host-specific Butane
includes. Manual delivery accepts production or test environments.
Cloudflare token/DNS lifecycle and opt-in VM provisioning are implemented.
Staging ACME and HTTPS acceptance passed on 2026-10-02; production renewal and
outside-tailnet access remain pending. See the
[implementation plan](../GENERIC-PRIVATE-UIS.md).

Skillet configures host-network Tailscale and manages tagged enrollment and
removal for clamps smoke VMs; live smoke enrollment and cleanup were accepted
on 2026-09-30. Caddy now has a generic Quadlet configuration for the UI
services declared by the host, using bridge DNS, persistent certificate storage, Cloudflare
DNS-01, and source-address restrictions for Tailscale's IPv4 and IPv6 ranges.
Its hostname and token credentials are separate from normal full apply.
Syncthing no longer publishes its GUI port on the host. Staging Caddy/ACME and
disposable token/DNS cleanup passed live acceptance. Real-client DNS/HTTPS,
production renewal, and denial from a separate network remain open in the UI
plan. The Syncthing ownership-fix runtime check is separately parked.

## Planned public services

## cloudflare-ui-lifecycle.md

Implementation status: `cloudflare-rs` supplies authenticated blocking HTTP
transport; custom typed JSON endpoints use that transport for token APIs and
DNS record comments that the crate's DNS models do not expose. Persistent
delivery use-or-creates and saves its scoped token in KeePassXC. The
workstation UI provisioner now persists disposable owner intent, issues and
journals its scoped token, reconciles DNS, records owned record IDs, and owns
disposable cleanup. Persistent delivery also owns tailnet device lookup,
ACME-token reuse or issuance, migration of the legacy production entry,
DNS reconciliation, complete credential-set delivery, and deferred Caddy
activation. Cleanup validates the recorded identity and current
environment configuration before mutation, removes marker-owned DNS, revokes
the disposable and temporary cleanup tokens, and removes its journal last.
Failures retain the journal for retry. Disposable guest delivery now installs
both credentials before Caddy activation, checks the explicit non-tailnet
denial response for every canonical UI and alias, and only then revokes older
tokens with the same deterministic name. Local
tests cover the SDK transport, permission group
discovery, scoped token payloads, address-family validation, and DNS planning.
Live 2026-10-02 staging acceptance issued certificates for each
declared canonical UI and alias, verified DNS and HTTPS over IPv4/IPv6, and
disposed the VM, DNS records, and child tokens. CI tests inject an ambiguous
token-create response, a DNS-create failure after a partial reconciliation,
and a DNS-delete failure during cleanup; each retry converges without
duplicates or deleting unrelated records. Opt-in smoke provisioning also
requests each configured UI from the guest's loopback source and requires
Caddy's explicit 403 response. This verifies the source-address matcher for a
non-tailnet source; denial from a separate external network and production
certificate acceptance remain pending.

## cloudflare-ddns.md

Decision, 2026-10-06: retain Rupik's public-address DDNS capability. Plan to
reuse the existing Chef image family in a rootful Quadlet container, with
caller opt-in and private configuration supplied through Skillet credentials.
This preserves the existing updater instead of introducing another guest
Cloudflare API client. Guest convergence, production credential delivery, and
journaled disposable token/record cleanup are implemented. Live disposable DNS
acceptance remains pending the test-only DDNS record-name entry.

The workstation issuer supplies a separate zone-scoped DDNS child token;
the token creator stays on the workstation. Deliver the complete config as an
encrypted credential and mount its Podman secret at the updater's config path.
Avoid a plaintext token-bearing config file on persistent host storage. Podman's
own disk secret copy remains covered by the planned storage encryption policy.
Use existing atomic delivery, deferred activation, and lifecycle ownership.

The selected image is upstream 2.2.0, pinned to its multi-platform index digest.
Its scratch image runs as UID/GID 0 with `/` as its working directory and
`/cloudflare-ddns --repeat` as entrypoint. The legacy JSON loader reads
`/config.json`; its repeat interval is the configured TTL (300 seconds).
Mount the complete Podman secret there as `0400`, UID/GID 0. Do not set
environment-mode token variables, which would bypass JSON configuration.
An offline run of the actual image accepted this secret mount with dummy
credentials and networking disabled. Digest pinning intentionally prevents
registry auto-update from changing this evaluated contract.
Its legacy API path reads one page of 100 A records; provisioning refuses a
zone that would exceed that limit after adding the declared records.

## email-delivery.md

Status: image packages and shared SMTP provisioning implemented. Native Postfix
container and rebuilt uCore capture acceptance passed. Live Mailjet submission
over verified STARTTLS and operator-confirmed Gmail receipt passed. Mail
authentication headers remain unverified; UniFi recovery email is parked by
operator decision.

Mailjet is the existing provider. Read a single `skillet/smtp` entry:
Username and Password contain SMTP credentials; `host`, `port` and `tls` custom
fields hold the endpoint and security mode; `sender` holds the verified sender
address. Field-aware reading, auditing and encrypted delivery are implemented.
The audit requires these fields once for production fleet preparation. Default test
provisioning neither reads nor delivers provider credentials. The explicit
`--smtp-only --live-smtp` acceptance override delivers real credentials through
the same owned VM transport without changing its test identity or enabling
other production services. It sends no message itself. Only `starttls` is
accepted; production requires certificate and hostname verification.
Missing/invalid field names are public diagnostics; values are never printed. SMTP credentials are the provider’s API key/secret, rather
than the account login ([provider guide](https://documentation.mailjet.com/hc/en-us/articles/360043229473-How-can-I-configure-my-SMTP-parameters)).

Relay only through an authenticated external provider, using verified TLS;
never fall back to direct destination-MX delivery. Keep the queue in persistent
`/var/spool/postfix`, independent of the service-data mount. Shared templates
hold policy; private configuration and credentials use the existing vault and
systemd encrypted-delivery workflow. A root-owned, Postfix-group-readable
`texthash` map (`0640`, parent `0750`) under `/run/postfix/skillet` is regenerated
from `LoadCredentialEncrypted=` before Postfix starts. Native Fedora acceptance
verified backend/worker permissions. Capture delivery and boot/rebase recovery
passed on uCore with enforcing SELinux, including production credential-map
access and live TLS/authentication. The guest preparation adapter copies the
Postfix configuration label onto RAM credential files when SELinux is enabled,
after restoring the reference file label. Disabled test containers skip labeling;
permissive hosts still receive correct labels. Trust uses Fedora’s extracted CA
bundle rather than assuming a compatibility symlink exists.
Skillet initializes the packaged queue layout and `/var/lib/postfix` before
startup, with package ownership/modes and restored SELinux labels. Image-layer
`/var` contents do not initialize a rebased host. Existing message contents are
preserved; managed directory metadata and security labels converge. Sender canonical rewriting preserves recipients.
Never bake credentials or sender domains into an image.

Default listener: loopback only. UniFi uses host networking and can reach it.
Bridge containers need an explicitly declared internal endpoint with restricted
access, preferably authenticated submission; do not expose port 25 on the LAN
or tailnet, or trust an entire bridge containing unrelated containers by default.
Application modules own their SMTP client integration and monitoring declarations.
Postfix and credential-preparation unit health are declared by the SMTP module;
queue-age and delivery-failure metrics remain pending.

## file-browsing.md

Decision, 2026-10-11: provide a shared file-management UI on Syncthing hosts,
with Caddy HTTPS and [verified tailnet identity](../../design/tailnet-ui-authentication.md).
The two intended users must enter without an internal password prompt. Serve
only caller-declared user folders; do not mount Syncthing configuration or the
whole data filesystem. Keep any app state outside synchronized files. The
magiusstaff second instance is deferred.

The requested [Awesome Selfhosted section](https://github.com/awesome-selfhosted/awesome-selfhosted#file-transfer---web-based-file-managers)
provided the initial shortlist; capabilities above were checked against upstream
projects. Mikochi's latest [1.12.0 release](https://github.com/zer0tonin/Mikochi/releases/tag/1.12.0)
was published 2026-09-25 and includes authentication fixes. Its documented single
login is not a per-user permission model: with `NO_AUTH=true`, the proxy allowlist
controls access and both users share the mounted scope. Confirm supported
operations and use a current pinned image in a disposable trial.

Real-container CI passed file operations and mapped ownership. The disposable
VM passed LocalAPI authorization, repeat apply and dual-stack backend isolation;
HTTPS browser acceptance is pending external ACME/connectivity checks. See
[the acceptance record](ACCEPTANCE.md). Broader symlink behavior remains
unverified; denial of an unmounted host target does not prove all container-path
containment.

## fleet-hosts.md

Decision, 2026-10-11: use joey for boxy's replacement and donbot for
calculon's replacement. The rename deliberately provides no selector, package
or image aliases. Current CLI selectors, binary/package names, templates and
image builds use these identities. Do not silently rename retained VM manifests,
provider ownership journals or vault entries; historical acceptance preserves
the identities actually tested. Existing disposable runs need an explicit
owned cleanup/migration path; new provisioning uses the renamed profiles and
requires the corresponding images to be built and published first.

Calculon/donbot source services, exclusions and public/mixed ingress needs are
in [the inventory](../DONBOT-INVENTORY.md) and
[migration plan](../DONBOT-MIGRATION.md). The donbot image recipe exists;
its Skillet profile, host binary and Butane configuration remain pending.


## ups.md

Decision, 2026-10-12: install Fedora `nut` and `nut-client` in the shared
uCore image, with their services/enumerator disabled by default. An optional
shared Skillet module owns `/etc/ups`, USB selection and native service
activation. Clamps selects the CyberPower CP1500 AVR (`0764:0501`) with
`usbhid-ups`; other host callers must explicitly opt in.

## host-networking.md

Physical boot selection, association and reachability remain acceptance gates.
Live connection switching/checkpoint rollback is not implemented. See
[setup](../../skillet/crates/network/README.md) and
[acceptance plan](../HOST-NETWORKING.md).

## btrbk.md

Status: implementation and real-VM snapshot/restore verification are complete.
See the [implementation milestone](CLAMPS-BTRBK.md) and
[shared storage design](../../design/storage.md).


## pihole-network.md

Publish Pi-hole DNS on host IPv4 and IPv6 port 53 for both TCP and UDP, matching
rupik. Keep its web listener on port 8088 inside the bridge; Caddy will proxy
to `http://pihole:8088` without publishing the admin port on the host. Pi-hole
uses `dns_listeningMode=ALL` for bridge-network clients. Mark its fully
qualified registry image for Quadlet auto-update; the OS image already enables
the auto-update timer. The planned private UI route and Cloudflare DNS-only
records are described in the [private UI design](../../design/private-ui-access.md).

The planned second Pi-hole on joey beside clamps is a separate DNS
instance on the same power and internet connection. A possible third on remote
donbot adds site diversity. All instances offered to the same clients must
serve consistent local records and filtering policy; the synchronization
mechanism and LAN client resolver settings remain to be decided before those
hosts are deployed.

## syncthing.md

Mount `/var/lib/data/syncthing` at `/var/syncthing`. Keep the upstream
directory layout there, including `config/` and synced folders, so a planned
state transfer can retain the device identity and folder definitions. The
directory is a Btrfs subvolume owned by the declared host account. Syncthing
runs as container UID/GID 1000; Podman's user namespace maps that container
identity to the host account and its primary group. Keep host ownership,
container identity, and namespace mapping explicit and separate. Never copy
a source server's live identity into a disposable VM.
