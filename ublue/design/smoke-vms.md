# Disposable smoke VM lifecycle

Decision: keep VM creation, readiness, repeatable smoke checks, and destruction
separate. A retained VM supports inspection and repeated convergence checks;
a fresh VM establishes that bootstrap is reproducible. The existing fixture
tests real systemd/Podman convergence. Pi-hole credential delivery and
Tailscale enrollment/device cleanup are implemented. Cloudflare DNS and live
ACME scenarios remain planned.

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

## States and recovery

1. **Create:** build the host binary, allocate the run directory and SSH key,
   and provision a fresh VM disk. Ignition contains the public SSH key and
   base configuration. Large binaries travel over SSH because embedding them
   in QEMU `fw_cfg` previously caused long boot delays.
2. **Ready:** wait for SSH, deliver the binary, complete unsigned then signed
   rebase, and check the booted deployment, base apply, resolver, SELinux, and
   user environment. Persistent bootstrap state tolerates reboots. Executable
   staging uses `/var/usrlocal/bin`, whose FCOS labeling permits execution.
   Ignition's authorized-key file remains configured alongside dotfiles keys.
   `ready` uses the artifact captured at creation. Explicit `update` builds
   and installs current host code on a retained VM, recording the deployed hash
   separately while preserving the original creation snapshot.
3. **Provision applications:** `test vm provision` unlocks the workstation
   KeePassXC database, reads the Tailscale OAuth client, mints a one-use smoke
   tagged key, and delivers it through the [shared secret mechanism](secrets.md).
   It waits for the VM to join and records its device identity before delivering
   a generated Pi-hole credential and running full apply. Repeated provisioning
   reuses the Tailscale identity and installed Pi-hole credential; `--rotate`
   replaces the latter.
4. **Smoke:** run assertions against the retained VM. Check repeat apply,
   configuration/secret changes, interruption recovery, and reboot persistence.
   The container sandbox uses mocked systemctl/Podman; only VM checks establish
   real runtime behavior. Failures retain the VM and diagnostics for repair.
5. **Destroy:** validate ownership, remove only the recorded smoke-tagged
   Tailscale device, then remove the domain, disk, SSH key, and artifacts.
   Failed Tailscale cleanup prevents local destruction and retains metadata for
   retry, even if the domain is already gone. Cloudflare records and tokens are
   not yet part of the lifecycle.

## Live ACME boundary

Routine smoke checks need no Cloudflare access. The optional live scenario
enrolls the VM in Tailscale, obtains its tailnet IP, and creates a DNS-only
A/AAAA record under the existing smoke-test Cloudflare zone using a per-VM
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
